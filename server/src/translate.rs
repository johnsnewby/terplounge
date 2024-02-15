use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use serde::{Deserialize, Serialize};

use crate::error::E;

pub trait Translator {
    fn translate(&self, req: TranslationRequest) -> E<()>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranslationRequest {
    pub session_id: usize,
    pub sequence_number: usize,
    pub payload: Vec<f32>,
    pub lang: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationResponse {
    pub sequence_number: usize,
    pub translation: String,
    pub num_segments: i32,
    pub segment_number: i32,
    pub segment_start: i64,
    pub segment_end: i64,
    pub uuid: String,
}

impl ToString for TranslationResponse {
    fn to_string(&self) -> String {
        self.translation.clone()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationResponses(Vec<Option<Vec<Option<TranslationResponse>>>>);

impl TranslationResponses {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn add_translation(&mut self, response: &TranslationResponse) -> E<()> {
        let sequence_number = response.sequence_number as usize;
        let segment_number = response.segment_number as usize;
        if self.0.len() < sequence_number + 1 {
            log::debug!(
                "Extending translation vector to {} length",
                sequence_number + 1
            );
            self.0.resize(sequence_number + 1, None);
        }
        let mut segments = match &self.0[sequence_number] {
            Some(x) => x.clone(),
            None => vec![],
        };
        log::debug!("segments: {:?}", segments);
        if segments.len() < segment_number + 1 {
            segments.resize(segment_number + 1, None);
            log::debug!("Extending segments vector to {} length", segment_number + 1);
        }
        segments[segment_number] = Some(response.clone());
        self.0[sequence_number] = Some(segments);
        Ok(())
    }

    pub fn translation_count(&self) -> E<usize> {
        let count = self.0.iter().filter(|x| !x.is_none()).count();
        Ok(count)
    }
}

impl ToString for TranslationResponses {
    fn to_string(&self) -> String {
        let mut result = String::new();

        for responses in self.0.iter() {
            match responses {
                Some(sequences) => {
                    for sequence in sequences.iter() {
                        match sequence {
                            Some(x) => result.push_str(&x.translation),
                            None => result.push_str(" ... "),
                        }
                    }
                }
                None => result.push_str(" .... "),
            }
        }
        result
    }
}

pub const SEND_SAMPLE_MINIMUM_TIME_SECONDS: usize = 15;
pub const SILENCE_TIME_MILLISECONDS: usize = 200;
pub const SILENCE_AMPLITUDE_THRESHOLD: f32 = 0.005;
//pub const SAMPLE_RATE: f64 = 44100f64;

/**
 * does what is says.
 */
pub fn resample(audio_data: &Vec<f32>, from_rate: f64) -> Vec<f32> {
    let mut resampler = SincFixedIn::<f32>::new(
        16000_f64 / from_rate,
        10.0,
        SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        },
        audio_data.len(),
        1,
    )
    .unwrap(); // TODO: handle properly

    let resampled = resampler.process(&[audio_data], None).unwrap();
    let data = &resampled[0];
    data.clone()
}

/**
 * detect silence. Firstly, only check 1/10 of requests, and only when
 * there are enough samples. Then take the last
 * SILENCE_TIME_MILLISECONDS and see if their average amplitude is
 * under SILENCE_AMPLITUDE_THRESHOLD. Return the buffer before the
 * pivot, and the part after to be retained for later processing TODO:
 * optimise!
*/
#[inline(always)]
pub fn find_silence(buffer: &[f32], sample_rate: u32) -> Option<usize> {
    let min_samples: usize = SEND_SAMPLE_MINIMUM_TIME_SECONDS * sample_rate as usize;
    let len = buffer.len();
    if len < min_samples {
        return None;
    }

    let mut total = 0f32;
    let silence_window: usize = sample_rate as usize * SILENCE_TIME_MILLISECONDS / 1000;
    let mut idx = min_samples;
    let mut num_samples: usize = 1;
    while idx < len {
        let sample = buffer[idx].abs();
        idx += 1;
        total += sample;
        let avg = total / num_samples as f32;
        num_samples += 1;
        if avg > (2.0 * SILENCE_AMPLITUDE_THRESHOLD) {
            // some hysteresis
            total = sample;
            num_samples = 1;
        } else if avg <= SILENCE_AMPLITUDE_THRESHOLD && num_samples > silence_window {
            return Some(idx - (silence_window / 2));
            // return the point in the middle of the silence, so the next sample
            // can ramp up slow.
        }
    }
    None
}
