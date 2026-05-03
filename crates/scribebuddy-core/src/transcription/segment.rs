use crate::{Speaker, TranscriptSegment};

impl TranscriptSegment {
    pub fn new(speaker: Speaker, text: String, start_time: i64, end_time: i64) -> Self {
        Self { speaker, text, start_time, end_time }
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn formatted_text(&self) -> String {
        let start = Self::format_secs(self.start_time);
        let text = self.text.trim();
        format!("**[{}] {}:**\n{}", start, self.speaker, text)
    }

    pub fn format_secs(secs: i64) -> String {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", h, m, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_segment() {
        let seg = TranscriptSegment::new(Speaker::You, "   ".into(), 0, 1);
        assert!(seg.is_empty());
    }

    #[test]
    fn test_formatted_text() {
        let seg = TranscriptSegment::new(Speaker::You, "Hello world".into(), 5, 7);
        let formatted = seg.formatted_text();
        assert!(formatted.contains("You"));
        assert!(formatted.contains("00:00:05"));
        assert!(formatted.contains("Hello world"));
    }
}
