use crate::error::{Er, E};
use crate::queue;
use crate::session::process_transcription;
use crate::translate::{resample, TranslationRequest, TranslationResponse, Translator};
use lazy_static::lazy_static;
use std::env;
use std::sync::OnceLock;
use thread_priority::set_current_thread_priority;
use thread_priority::ThreadPriority::Crossplatform;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

lazy_static! {
    static ref CTX: OnceLock<WhisperContext> = {
        let model = env::var("WHISPER_MODEL").unwrap_or("medium".to_string());
        let ctx = WhisperContext::new(&format!("../models/ggml-{}.bin", model)).unwrap();
        let lock = OnceLock::new();
        lock.set(ctx).unwrap();
        lock
    };
}

pub struct WhisperCpp {}

impl WhisperCpp {}

impl Translator for WhisperCpp {
    fn translate(&self, translation_request: TranslationRequest) -> E<()> {
        log::debug!(
            "Sending job {} to translate",
            &translation_request.session_id
        );
        let session = match crate::session::get_session_sync(&translation_request.session_id) {
            Some(x) => x,
            None => {
                return Err(Er::new(format!(
                    "Couldn't get session for request {:?}",
                    translation_request
                )))
            }
        };

        let audio_data = translation_request.payload;

        let data = resample(&audio_data, 44100_f64);

        let mut bytes: Vec<u8> = Vec::with_capacity(4 * data.len());
        for val in &data {
            bytes.extend(&val.to_le_bytes());
        }

        let context = CTX.get().expect("Couldn't get context");
        let mut state = context.create_state().expect("failed to create state");
        let mut whisper_params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        log::debug!("Setting language to {}", translation_request.lang);
        whisper_params.set_language(Some(&translation_request.lang));
        state
            .full(whisper_params, &data)
            .expect("failed to run model");

        let num_segments = state
            .full_n_segments()
            .expect("failed to get number of segments");
        log::debug!("{} segments", num_segments);
        for i in 0..num_segments {
            let segment = match state.full_get_segment_text(i) {
                Ok(text) => text,
                Err(_) => "<b>error transcribing</b>".to_string(),
            };
            let start_timestamp = state
                .full_get_segment_t0(i)
                .expect("failed to get start timestamp");
            let end_timestamp = state
                .full_get_segment_t1(i)
                .expect("failed to get end timestamp");

            log::debug!("[{} - {}]: {}", start_timestamp, end_timestamp, segment);

            let response = TranslationResponse {
                sequence_number: translation_request.sequence_number,
                translation: segment,
                num_segments,
                segment_number: i,
                segment_start: start_timestamp,
                segment_end: end_timestamp,
                uuid: session.uuid.to_string(),
            };

            let result = process_transcription(translation_request.session_id, &response);
            match result {
                Ok(_) => (),
                Err(e) => {
                    log::warn!("Processing translation failed with error {}", e);
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

pub fn start_translate_pool() -> E<()> {
    let num_whisper_processes: usize = match env::var("WHISPER_PROCESSES") {
        Ok(num) => num.parse().expect("WHISPER_PROCESSES must be an integer"),
        Err(_) => {
            let num_cpus = num_cpus::get();
            num_cpus / 4
        }
    };
    log::debug!("Making thread pool with {} threads.", num_whisper_processes);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_whisper_processes)
        .build()
        .expect("Failed to create thread pool");
    let queue = queue::get_queue();

    for i in 0..num_whisper_processes {
        log::debug!("Installing {}", i);
        let mut queue = queue.clone();
        set_current_thread_priority(Crossplatform(crate::LOWER_PRIORITY.try_into().unwrap()))
            .unwrap();
        pool.spawn(move || {
            queue
                .subscribe::<WhisperCpp>(&WhisperCpp {})
                .unwrap_or_else(|_| {
                    log::warn!("Exiting thread");
                });
        });
    }
    Ok(())
}
