use crate::{Speaker, TranscriptSegment};
use chrono::Duration;

impl TranscriptSegment {
    pub fn new(speaker: Speaker, text: String, start_time: Duration, end_time: Duration) -> Self {
        Self {
            speaker,
            text,
            start_time,
            end_time,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn formatted_text(&self) -> String {
        let start = Self::format_duration(self.start_time);
        let text = self.text.trim();
        format!("**[{}] {}:**\n{}", start, self.speaker, text)
    }

    fn format_duration(d: Duration) -> String {
        let total_secs = d.num_seconds();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_segment() {
        let seg = TranscriptSegment::new(Speaker::You, "   ".into(), Duration::seconds(0), Duration::seconds(1));
        assert!(seg.is_empty());
    }

    #[test]
    fn test_formatted_text() {
        let seg = TranscriptSegment::new(Speaker::You, "Hello world".into(), Duration::seconds(5), Duration::seconds(7));
        let formatted = seg.formatted_text();
        assert!(formatted.contains("You"));
        assert!(formatted.contains("00:00:05"));
        assert!(formatted.contains("Hello world"));
    }
}
