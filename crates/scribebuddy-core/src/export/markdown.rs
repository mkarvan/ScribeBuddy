use crate::TranscriptSegment;
use chrono::Local;

pub struct MarkdownExporter;

impl MarkdownExporter {
    pub fn export(segments: &[TranscriptSegment]) -> String {
        let now = Local::now();
        let total_secs = segments.last().map(|s| s.end_time).unwrap_or(0);

        let mut md = String::new();

        md.push_str("# Meeting Transcript\n\n");
        md.push_str(&format!("**Date:** {}\n", now.format("%Y-%m-%d")));
        md.push_str(&format!(
            "**Duration:** {}\n",
            TranscriptSegment::format_secs(total_secs)
        ));
        md.push_str("\n---\n\n");

        for segment in segments {
            if segment.is_empty() {
                continue;
            }
            md.push_str(&segment.formatted_text());
            md.push_str("\n\n");
        }

        md.push_str("---\n");
        md.push_str("*Transcribed by ScribeBuddy*\n");

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Speaker, TranscriptSegment};

    #[test]
    fn test_export_format() {
        let segments = vec![
            TranscriptSegment::new(Speaker::You, "Hello team".into(), 5, 7),
            TranscriptSegment::new(Speaker::Remote, "Hi there".into(), 12, 15),
        ];

        let md = MarkdownExporter::export(&segments);

        assert!(md.contains("# Meeting Transcript"));
        assert!(md.contains("**Date:**"));
        assert!(md.contains("**Duration:**"));
        assert!(md.contains("**[00:00:05] You:**"));
        assert!(md.contains("**[00:00:12] Remote:**"));
        assert!(md.contains("*Transcribed by ScribeBuddy*"));
    }

    #[test]
    fn test_empty_segments_excluded() {
        let segments = vec![
            TranscriptSegment::new(Speaker::You, "   ".into(), 0, 1),
        ];
        let md = MarkdownExporter::export(&segments);
        assert!(!md.contains("You"));
    }
}
