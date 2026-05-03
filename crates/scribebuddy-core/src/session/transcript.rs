use crate::TranscriptSegment;
use crossbeam_channel::Receiver;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct TranscriptAccumulator {
    segments: Arc<RwLock<Vec<TranscriptSegment>>>,
}

impl TranscriptAccumulator {
    pub fn new() -> Self {
        Self {
            segments: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_segment(&self, segment: TranscriptSegment) {
        if segment.is_empty() {
            return;
        }
        let mut segments = self.segments.write();
        segments.push(segment);
    }

    pub fn get_segments(&self) -> Vec<TranscriptSegment> {
        self.segments.read().clone()
    }

    pub fn count(&self) -> usize {
        self.segments.read().len()
    }

    pub fn listen_channel(&self, rx: Receiver<TranscriptSegment>) {
        let segments = self.segments.clone();
        std::thread::spawn(move || {
            while let Ok(segment) = rx.recv() {
                segments.write().push(segment);
            }
        });
    }
}

impl Default for TranscriptAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Speaker;
    use chrono::Duration;

    #[test]
    fn test_add_and_retrieve() {
        let acc = TranscriptAccumulator::new();
        let seg = TranscriptSegment::new(
            Speaker::You,
            "test".into(),
            Duration::seconds(0),
            Duration::seconds(2),
        );
        acc.add_segment(seg);
        assert_eq!(acc.count(), 1);
        assert_eq!(acc.get_segments()[0].text, "test");
    }

    #[test]
    fn test_empty_segment_ignored() {
        let acc = TranscriptAccumulator::new();
        let seg = TranscriptSegment::new(
            Speaker::You,
            "   ".into(),
            Duration::seconds(0),
            Duration::seconds(1),
        );
        acc.add_segment(seg);
        assert_eq!(acc.count(), 0);
    }
}
