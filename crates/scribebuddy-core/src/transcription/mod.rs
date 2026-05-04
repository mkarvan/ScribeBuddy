pub mod engine;
pub mod model;
pub mod segment;

use crate::{Speaker, TranscriptSegment};

pub trait Transcriber: Send {
    fn transcribe(
        &mut self,
        samples: &[f32],
        speaker: Speaker,
        offset_secs: i64,
    ) -> anyhow::Result<Vec<TranscriptSegment>>;
}
