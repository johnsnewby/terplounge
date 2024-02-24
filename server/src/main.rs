mod api;
mod compare;
mod error;
mod metadata;
mod queue;
mod session;
mod translate;
mod whispercpp;
mod whisperx;

use crossbeam_channel::unbounded;
use dotenv::dotenv;
use thread_priority::*;

use crate::api::serve;
use crate::whisperx::WhisperX;

pub const LOWER_PRIORITY: u8 = 40;
pub const HIGHER_PRIORITY: u8 = 60;

#[tokio::main]
async fn main() {
    dotenv().ok();

    env_logger::init();

    let (_translate_tx, translate_rx) = unbounded();
    log::debug!("Making transcription pool");
    whispercpp::start_translate_pool().unwrap();
    log::debug!("Made WhisperCpp pool");
    if std::env::var("WHISPER_SERVER").is_ok() {
        std::thread::spawn(move || async {
            set_current_thread_priority(ThreadPriority::Crossplatform(
                HIGHER_PRIORITY.try_into().unwrap(),
            ))
            .unwrap();
            let mut queue = queue::get_queue();
            let whisperx = WhisperX::new().unwrap();
            log::debug!("Waiting for WhisperX job");
            queue.subscribe::<WhisperX>(&whisperx).unwrap();
        });
        log::debug!("Started remote whisper process");
    }
    log::info!("Restoring old sessions");
    crate::session::restore_sessions().await.unwrap();

    std::thread::spawn(move || async { queue::get_queue().queue_process(translate_rx).await });
    log::debug!("Made enqueuing process");
    serve().await;
}
