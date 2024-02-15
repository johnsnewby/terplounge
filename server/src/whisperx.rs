use log::debug;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use warp::ws::Message;

use crate::error::{Er, E};
use crate::translate::{resample, TranslationRequest, TranslationResponse, Translator};

#[derive(Deserialize, Debug)]
struct RemoteWhisperSegment {
    text: String,
    start: f32,
    end: f32,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct RemoteWhisperResponse {
    segments: Vec<RemoteWhisperSegment>,
    language: String,
}

pub struct WhisperX {
    client: Client,
}

impl WhisperX {
    pub fn new() -> E<Self> {
        let client = Client::new();
        Ok(Self { client })
    }
}

impl Translator for WhisperX {
    fn translate(&self, translation_request: TranslationRequest) -> E<()> {
        let audio_data = translation_request.payload;
        if audio_data.is_empty() {
            return Ok(());
        }
        let data = resample(&audio_data, 44100_f64);

        let url = format!(
            "{}?lang={}",
            std::env::var("WHISPER_SERVER").unwrap(),
            translation_request.lang
        );
        let session_id = translation_request.session_id;
        let session = crate::session::get_session_sync(&translation_request.session_id).ok_or(
            Er::new(format!("Couldn't get session for request {:?}", session_id)),
        )?;
        debug!("Making request for translation to {}", url);

        let res = self.client.post(url).json(&json!(data)).send()?;
        let response = res.json::<RemoteWhisperResponse>()?;

        for segment in response.segments {
            let response = TranslationResponse {
                sequence_number: translation_request.sequence_number,
                translation: segment.text,
                num_segments: 1,
                segment_number: 0,
                segment_start: (segment.start * 1000f32) as i64,
                segment_end: (segment.end * 1000f32) as i64,
                uuid: session.uuid.to_string(),
            };

            match session
                .transcription_sender_tx
                .as_ref()
                .expect("Couldn't find sender!")
                .send(Message::text(json!(response).to_string()))
            {
                Ok(_) => (),
                Err(e) => {
                    log::warn!("Sending failed with error {}", e);
                    crate::session::mutate_session_sync(
                        &translation_request.session_id,
                        |session| session.valid = false,
                    );
                }
            };
        }
        Ok(())
    }
}
