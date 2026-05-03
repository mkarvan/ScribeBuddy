use crate::TranscriptSegment;
use chrono::{Duration, Local};

pub struct MarkdownExporter;

impl MarkdownExporter {
    pub fn export(segments: &[TranscriptSegment]) -> String {
        let now = Local::now();
        let total_duration = segments
            .last()
            .map(|s| s.end_time)
            .unwrap_or(Duration::zero());

        let mut md = String::new();

        md.push_str("# Meeting Transcript\n\n");
        md.push_str(&format!(
            "**Date:** {}\n",
            now.format("%Y-%m-%d")
        ));
        md.push_str(&format!(
            "**Duration:** {}\n",
            format_duration(total_duration)
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

fn format_duration(d: Duration) -> String {
    let total_secs = d.num_seconds();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Speaker, TranscriptSegment};
    use chrono::Duration;

    #[test]
    fn test_export_format() {
        let segments = vec![
            TranscriptSegment::new(Speaker::You, "Hello team".into(), Duration::seconds(5), Duration::seconds(7)),
            TranscriptSegment::new(Speaker::Remote, "Hi there".into(), Duration::seconds(12), Duration::seconds(15)),
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
            TranscriptSegment::new(Speaker::You, "   ".into(), Duration::seconds(0), Duration::seconds(1)),
        ];
        let md = MarkdownExporter::export(&segments);
        assert!(!md.contains("You"));
    }
}
