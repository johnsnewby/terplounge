use chrono::{DateTime, Utc};
use crossbeam_channel::{unbounded, Sender};
use futures_util::{SinkExt, StreamExt};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::RwLock;
use tokio::time::timeout;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};

const RECV_TIMEOUT_SECONDS: u64 = 15;

use crate::error::E;
use crate::queue::{self};
use crate::translate::{self, TranslationResponse, TranslationResponses};

pub type Sessions = HashMap<usize, SessionData>;

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Debug, Serialize)]
pub struct SessionData {
    #[serde(skip_serializing)]
    id: usize,
    #[serde(skip_serializing)]
    pub transcription_sender_tx: Option<Sender<Message>>,
    #[serde(skip_serializing)]
    pub translator: Sender<translate::TranslationRequest>,
    pub language: String,
    pub uuid: Uuid,
    pub resource: Option<String>,
    pub sample_rate: u32,
    #[serde(skip_serializing)]
    pub valid: bool,
    #[serde(skip_serializing)]
    pub buffer: Vec<f32>,
    #[serde(skip_serializing)]
    pub silence_length: usize,
    pub sequence_number: usize,
    #[serde(skip_serializing)]
    pub last_sequence: Option<usize>,
    #[serde(skip_serializing)]
    pub recording: bool,
    #[serde(skip_serializing)]
    pub recording_file: Option<String>,
    #[serde(skip_serializing)]
    pub transcript_file: Option<String>,
    #[serde(skip_serializing)]
    pub translations: Arc<Mutex<TranslationResponses>>,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize)]
struct SavedSessionData {
    pub language: String,
    pub uuid: Uuid,
    pub resource: Option<String>,
    pub sample_rate: u32,
    pub valid: bool,
    pub silence_length: usize,
    pub sequence_number: usize,
    pub last_sequence: Option<usize>,
    pub recording: bool,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl SessionData {
    fn new(
        id: usize,
        transcription_sender_tx: Sender<Message>,
        translator: Sender<translate::TranslationRequest>,
        language: String,
        sample_rate: u32,
        resource: Option<String>,
        _uuid: Option<Uuid>,
    ) -> Self {
        let uuid = if let Some(u) = _uuid {
            u
        } else {
            Uuid::new_v4()
        };
        let mut recording_file = None;
        let mut transcript_file = None;
        if let Ok(dir) = std::env::var("RECORDINGS_DIR") {
            let new_dir = format!("{}/{}", dir, uuid);
            if std::fs::create_dir_all(new_dir.clone()).is_ok() {
                recording_file = Some(format!("{}/{}.wav", new_dir, uuid));
                transcript_file = Some(format!("{}/{}.txt", new_dir, uuid));
            }
        };
        Self {
            id,
            transcription_sender_tx: Some(transcription_sender_tx),
            translator,
            language,
            sample_rate,
            silence_length: 0usize,
            uuid,
            resource,
            recording: recording_file.is_some(),
            recording_file,
            transcript_file,
            valid: true,
            buffer: Vec::new(),
            sequence_number: 0,
            last_sequence: None,
            translations: Arc::new(Mutex::new(TranslationResponses::new())),
            updated_at: Utc::now(),
            created_at: Utc::now(),
        }
    }

    pub fn get_translation_count(&self) -> E<usize> {
        let mutex = self.translations.lock().unwrap();
        let responses: &crate::translate::TranslationResponses = mutex.deref();
        let count = responses.translation_count()?;

        Ok(count)
    }

    pub fn send_uuid(&mut self) -> E<()> {
        self.transcription_sender_tx
            .as_ref()
            .ok_or("couldn't find sender")?
            .send(Message::text(
                json!({ "uuid": self.uuid.to_string() }).to_string(),
            ))?;
        Ok(())
    }

    pub fn transcript(&self) -> E<String> {
        let mutex = self.translations.lock().unwrap();
        let responses: &crate::translate::TranslationResponses = mutex.deref();
        Ok(responses.to_string())
    }

    pub fn finalize_session(&mut self) {
        self.record_transcript()
            .expect("error recording transcript");
        self.write_metadata().expect("error writing metadata");
        mutate_session_sync(&self.id, |session| {
            let sender = session.transcription_sender_tx.take();
            drop(sender);
            session.valid = false;
            log::debug!("good bye user: {}", session.id);
        });
    }

    fn write_metadata(&self) -> E<()> {
        if let Ok(dir) = std::env::var("RECORDINGS_DIR") {
            if let metadata_file = Some(format!("{}/{}/metadata.json", dir, self.uuid)) {
                let mut file = std::fs::File::create(metadata_file.unwrap())?;
                let json = json!(self).to_string();
                file.write_all(json.as_bytes())?;
            }
        }
        Ok(())
    }

    fn record_transcript(&self) -> E<()> {
        if let Some(filename) = &self.transcript_file {
            let mut file = std::fs::File::create(filename)?;
            let transcript = self.transcript()?;
            log::debug!("writing transcript: {}", transcript);
            file.write_all(transcript.as_bytes())?;
        }
        Ok(())
    }
}

lazy_static! {
    static ref WEBSOCKET_SEND_RUNTIME: Runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("user-runtime")
        .thread_stack_size(3 * 1024 * 1024)
        .build()
        .unwrap();
    static ref SYNC_BRIDGE_RUNTIME: Runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("sync-bridge-runtime")
        .thread_stack_size(3 * 1024 * 1024)
        .build()
        .unwrap();
    pub static ref SESSIONS: RwLock<Sessions> = RwLock::new(Sessions::default());
}

pub fn process_transcription(session_id: usize, response: &TranslationResponse) -> E<()> {
    let mut session = get_session_sync(&session_id).unwrap();
    log::debug!(
        "Sending {:?} to user\nSessionData is {}, last_sequence = {:?}",
        response,
        json!(session).to_string(),
        session.last_sequence,
    );
    session
        .transcription_sender_tx
        .as_ref()
        .ok_or("couldn't find sender")?
        .send(Message::text(json!(response).to_string()))?;
    session
        .translations
        .lock()
        .unwrap()
        .deref_mut()
        .add_translation(&response.clone())?;

    if let Some(last) = session.last_sequence
        && session.sequence_number >= last
        && response.segment_number == response.num_segments - 1
        && let Ok(translation_count) = session.get_translation_count()
        && translation_count >= last + 1
    {
        log::debug!(
            "Last sequence set and reached. Finalizing session {}.",
            session_id
        );
        session.finalize_session();
    }
    Ok(())
}

pub async fn get_session(id: &usize) -> Option<SessionData> {
    SESSIONS.write().await.get(id).cloned()
}

pub async fn get_sessions() -> Option<Vec<SessionData>> {
    Some(SESSIONS.read().await.iter().map(|x| x.1.clone()).collect())
}

pub fn get_session_sync(id: &usize) -> Option<SessionData> {
    let mut session: Option<SessionData> = None;
    SYNC_BRIDGE_RUNTIME.block_on(async {
        session = SESSIONS.write().await.get(id).cloned();
    });
    session
}

async fn set_session(id: usize, session: SessionData) {
    SESSIONS.write().await.insert(id, session);
}

// returns the id of the session with given uuid.
pub async fn find_session_with_uuid(uuid: &String) -> Option<usize> {
    for element in SESSIONS.read().await.iter() {
        if element.1.uuid.to_string().eq(uuid) {
            return Some(*element.0);
        }
    }
    None
}

pub async fn mutate_session<F>(id: &usize, mut f: F)
where
    F: FnMut(&mut SessionData),
{
    if let Some(x) = SESSIONS.write().await.get_mut(id) {
        f(x);
        x.updated_at = Utc::now();
    }
}

pub fn mutate_session_sync<F>(id: &usize, f: F)
where
    F: FnMut(&mut SessionData),
{
    SYNC_BRIDGE_RUNTIME.block_on(async {
        mutate_session(id, f).await;
    });
}

async fn remove_session(id: &usize) {
    let mut sessions = SESSIONS.write().await;
    sessions.remove(id);
}

pub async fn user_message(session_id: usize, msg: Message) -> E<()> {
    if !msg.is_binary() {
        // TODO: handle this
        return Ok(());
    }
    let data = msg.into_bytes();
    if let Some(session) = get_session(&session_id).await
        && let Some(ref _transcription_sender_tx) = session.transcription_sender_tx
    {
        let mut v: Vec<f32> = data
            .chunks_exact(4)
            .map(|a| f32::from_le_bytes([a[0], a[1], a[2], a[3]]))
            .collect();

        mutate_session(&session_id, |session| session.buffer.append(&mut v)).await;

        if let Some(pivot) = translate::find_silence(&session.buffer, session.sample_rate) {
            log::debug!(
                "Comparing {} to {}",
                pivot,
                crate::translate::SEND_SAMPLE_MINIMUM_TIME_SECONDS * session.sample_rate as usize
            );
            let silence_length = if pivot
                == crate::translate::SEND_SAMPLE_MINIMUM_TIME_SECONDS * session.sample_rate as usize
            {
                log::debug!("Silent for {} samples.", session.silence_length);
                session.silence_length + pivot
            } else {
                0
            };

            log::debug!("Sending to translate, pivot={}", pivot);
            let sequence_number = session.sequence_number;
            let payload = session.buffer[..pivot].to_vec();
            let lang = session.language.clone();
            persist_session_data(&session, pivot)?;
            let result = queue::get_queue().enqueue(translate::TranslationRequest {
                session_id,
                sequence_number,
                payload,
                lang,
            });
            match result {
                Ok(_) => {
                    drop(result);
                    mutate_session(&session_id, |session| {
                        session.silence_length = silence_length;
                        session.buffer = session.buffer[pivot..].to_vec();
                        session.sequence_number += 1;
                    })
                    .await;
                }
                Err(_) => {
                    drop(result);
                    mutate_session(&session_id, |session| {
                        session.transcription_sender_tx = None;
                        session.valid = false;
                    })
                    .await;
                }
            }
        }
    }
    Ok(())
}

pub async fn user_connected(
    ws: WebSocket,
    translate_tx: Sender<translate::TranslationRequest>,
    lang: String,
    sample_rate: u32,
    resource: Option<String>,
) {
    let session_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    log::debug!("new chat user: {}", session_id);

    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    let (transcription_send_tx, transcript_receive_rx) = unbounded();
    (*WEBSOCKET_SEND_RUNTIME).spawn(async move {
        for message in transcript_receive_rx.iter() {
            log::debug!("Sending message");
            match user_ws_tx.send(message).await {
                Ok(_) => (),
                Err(e) => {
                    log::debug!("websocket send error: {}", e);
                    break;
                }
            }
        }
        log::debug!("Exiting loop");
        user_ws_tx.close().await.unwrap();
    });

    let mut session = SessionData::new(
        session_id,
        transcription_send_tx,
        translate_tx,
        lang,
        sample_rate,
        resource,
        None,
    );
    session.send_uuid().unwrap();
    set_session(session_id, session).await;

    loop {
        if let Ok(Some(result)) =
            timeout(Duration::from_secs(RECV_TIMEOUT_SECONDS), user_ws_rx.next()).await
        {
            let msg = match result {
                Ok(msg) => msg,
                Err(e) => {
                    log::debug!("websocket error(uid={}): {}", session_id, e);
                    break;
                }
            };

            let session = get_session(&session_id).await;
            match session {
                Some(s) => {
                    if !s.valid && s.get_translation_count().unwrap() == s.last_sequence.unwrap() {
                        break;
                    }
                }
                None => {
                    log::warn!("Error getting session {}, bailing", session_id);
                    break;
                }
            }
            let _ = user_message(session_id, msg).await;
        } else {
            // timed out or error receiving
            break;
        }
    }
    log::debug!("Marking session {} for closure", session_id);
    mark_session_for_closure(session_id).await;
    drop(user_ws_rx);
    log::debug!("Exiting user_connected event loop");
}

pub async fn mark_session_for_closure_uuid(uuid: String) {
    if let Some(session_id) = find_session_with_uuid(&uuid).await {
        mark_session_for_closure(session_id).await;
    }
}

pub async fn mark_session_for_closure(session_id: usize) {
    let session = get_session(&session_id).await.unwrap();
    if session.sequence_number == 0 {
        mutate_session(&session_id, |session| {
            session.transcription_sender_tx = None;
        })
        .await;
        return;
    }
    let last_sequence = session.sequence_number - 1;
    log::debug!(
        "Found session {}, marking it for closure at sequence number {}",
        session_id,
        last_sequence,
    );
    mutate_session(&session_id, |session| {
        session.last_sequence = Some(last_sequence)
    })
    .await;
}

pub async fn expire_sessions() -> E<()> {
    let now = Utc::now().timestamp();
    for (session_id, session) in (*SESSIONS).read().await.iter() {
        if now - session.updated_at.timestamp() > 86400 {
            remove_session(session_id).await;
        }
    }
    Ok(())
}

fn persist_session_data(session: &SessionData, pivot: usize) -> E<()> {
    if let Some(filename) = &session.recording_file {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: session.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = if std::path::Path::exists(std::path::Path::new(&filename)) {
            hound::WavWriter::append(filename)?
        } else {
            hound::WavWriter::create(filename, spec)?
        };
        for sample in &session.buffer[..pivot] {
            writer.write_sample(*sample).unwrap();
        }
    }

    Ok(())
}

fn restore_sessions() -> E<Vec<SavedSessionData>> {
    let mut sessions: Vec<SavedSessionData> = vec![];
    if let Ok(dir) = std::env::var("RECORDINGS_DIR") {
        for entry in std::fs::read_dir(dir.clone())? {
            let entry = entry?;
            if entry.metadata()?.is_dir() {
                if let Ok(contents) = std::fs::read_to_string(format!(
                    "{}/{}/metadata.json",
                    dir,
                    entry.file_name().to_str().expect("Could not get filename!")
                )) {
                    let saved: SavedSessionData = serde_json::from_str(&contents)?;
                    sessions.push(saved);
                }
            }
        }
    }
    Ok(sessions)
}
