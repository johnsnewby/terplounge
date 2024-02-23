use crate::error::E;
use crossbeam_channel::{unbounded, Receiver, Sender};
use lazy_static::lazy_static;

use crate::translate::{TranslationRequest, Translator};

#[derive(Clone)]
pub struct TranslationQueue {
    sender: Sender<TranslationRequest>,
    receiver: Option<Receiver<TranslationRequest>>,
}

lazy_static! {
    pub static ref QUEUE: TranslationQueue = TranslationQueue::new().unwrap();
}

impl TranslationQueue {
    pub fn new() -> E<Self> {
        let (sender, receiver) = unbounded();
        Ok(Self {
            sender,
            receiver: Some(receiver),
        })
    }

    pub fn enqueue(&self, request: TranslationRequest) -> E<()> {
        log::debug!(
            "Enqueuing request for session with id {}",
            request.session_id
        );
        self.sender.send(request)?;
        log::debug!("Done");
        Ok(())
    }

    pub async fn queue_process(&self, rx: Receiver<TranslationRequest>) -> E<()> {
        for translation_request in rx.iter() {
            self.sender.send(translation_request)?;
        }
        Ok(())
    }

    pub fn subscribe<T: Translator>(&mut self, translator: &T) -> E<()> {
        while let Some(receiver) = &self.receiver {
            let req = receiver.recv()?;
            let session_id = req.session_id;
            log::debug!("Queue length: {}", receiver.len());
            if let Some(session) = crate::session::get_session_sync(&session_id) {
                if session.valid {
                    log::debug!(
                        "Sending job from session {} to translate, sequence_number is {}.",
                        session_id,
                        session.sequence_number
                    );
                    translator.translate(req)?;
                } else {
                    log::debug!("Skipping no longer valid session {}", session_id);
                }
            }
            log::warn!("Couldn't load session with id {}", &session_id);
        }
        log::debug!("Receiver closed.");
        Ok(())
    }
}

pub fn get_queue() -> TranslationQueue {
    (*QUEUE).clone()
}
