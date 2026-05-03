pub mod audio;
pub mod export;
pub mod session;
pub mod transcription;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Speaker {
    You,
    Remote,
}

impl std::fmt::Display for Speaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Speaker::You => write!(f, "You"),
            Speaker::Remote => write!(f, "Remote"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub speaker: Speaker,
    pub text: String,
    /// Seconds since session start (integer, serializes as a plain JSON number).
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionState {
    Idle,
    Recording,
    Paused,
    Stopped,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionState::Idle => write!(f, "Idle"),
            SessionState::Recording => write!(f, "Recording"),
            SessionState::Paused => write!(f, "Paused"),
            SessionState::Stopped => write!(f, "Stopped"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
    Medium,
    /// Large-v3 — always multilingual, highest accuracy.
    Large,
}

impl ModelSize {
    /// Returns the GGML model filename.
    /// `multilingual = false` uses the faster English-only `.en.bin` variant.
    /// Large is always multilingual regardless of the flag.
    pub fn filename(&self, multilingual: bool) -> String {
        match self {
            ModelSize::Large => "ggml-large-v3.bin".to_string(),
            size => {
                let base = match size {
                    ModelSize::Tiny => "ggml-tiny",
                    ModelSize::Base => "ggml-base",
                    ModelSize::Small => "ggml-small",
                    ModelSize::Medium => "ggml-medium",
                    ModelSize::Large => unreachable!(),
                };
                if multilingual {
                    format!("{}.bin", base)
                } else {
                    format!("{}.en.bin", base)
                }
            }
        }
    }

    /// HuggingFace download URL for the model file.
    pub fn download_url(&self, multilingual: bool) -> String {
        let filename = self.filename(multilingual);
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            filename
        )
    }
}

impl std::default::Default for ModelSize {
    fn default() -> Self {
        ModelSize::Small
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub target_app_bundle_id: Option<String>,
    pub chunk_duration_secs: f32,
    pub model_size: ModelSize,
    pub use_screencapturekit: bool,
    pub silence_threshold_rms: f32,
    /// BCP-47 language code for transcription (e.g. "en", "es", "fr").
    /// Use "auto" or leave empty to let Whisper auto-detect the language.
    /// Non-"en" values require a multilingual model (no .en suffix).
    pub language: String,
}

impl SessionConfig {
    /// Returns true when the multilingual model variant should be used.
    /// Large is always multilingual; any non-English language triggers multilingual.
    pub fn is_multilingual(&self) -> bool {
        self.model_size == ModelSize::Large || self.language != "en"
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            target_app_bundle_id: None,
            chunk_duration_secs: 3.0,
            model_size: ModelSize::default(),
            use_screencapturekit: true,
            silence_threshold_rms: 0.003,
            language: "en".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningApp {
    pub bundle_id: String,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_config_default_values() {
        let c = SessionConfig::default();
        assert_eq!(c.chunk_duration_secs, 3.0);
        assert_eq!(c.model_size, ModelSize::Small);
        assert!(c.use_screencapturekit);
        assert_eq!(c.silence_threshold_rms, 0.003);
        assert_eq!(c.language, "en");
        assert!(!c.is_multilingual());
    }

    #[test]
    fn is_multilingual_for_non_english() {
        let mut c = SessionConfig::default();
        c.language = "es".to_string();
        assert!(c.is_multilingual());
    }

    #[test]
    fn is_multilingual_for_auto() {
        let mut c = SessionConfig::default();
        c.language = "auto".to_string();
        assert!(c.is_multilingual());
    }

    #[test]
    fn large_is_always_multilingual() {
        let mut c = SessionConfig::default();
        c.model_size = ModelSize::Large;
        c.language = "en".to_string();
        assert!(c.is_multilingual());
    }

    #[test]
    fn model_size_filename_english() {
        assert_eq!(ModelSize::Tiny.filename(false), "ggml-tiny.en.bin");
        assert_eq!(ModelSize::Base.filename(false), "ggml-base.en.bin");
        assert_eq!(ModelSize::Small.filename(false), "ggml-small.en.bin");
        assert_eq!(ModelSize::Medium.filename(false), "ggml-medium.en.bin");
        assert_eq!(ModelSize::Large.filename(false), "ggml-large-v3.bin");
    }

    #[test]
    fn model_size_filename_multilingual() {
        assert_eq!(ModelSize::Tiny.filename(true), "ggml-tiny.bin");
        assert_eq!(ModelSize::Base.filename(true), "ggml-base.bin");
        assert_eq!(ModelSize::Small.filename(true), "ggml-small.bin");
        assert_eq!(ModelSize::Medium.filename(true), "ggml-medium.bin");
        assert_eq!(ModelSize::Large.filename(true), "ggml-large-v3.bin");
    }

    #[test]
    fn model_size_download_url_english() {
        let url = ModelSize::Small.download_url(false);
        assert!(url.contains("ggml-small.en.bin"), "url: {}", url);
        assert!(url.starts_with("https://huggingface.co/"));
    }

    #[test]
    fn model_size_download_url_multilingual() {
        let url = ModelSize::Small.download_url(true);
        assert!(url.contains("ggml-small.bin"), "url: {}", url);
        assert!(!url.contains(".en.bin"), "url: {}", url);
    }

    #[test]
    fn model_size_json_round_trip() {
        for size in [
            ModelSize::Tiny,
            ModelSize::Base,
            ModelSize::Small,
            ModelSize::Medium,
            ModelSize::Large,
        ] {
            let json = serde_json::to_string(&size).unwrap();
            let back: ModelSize = serde_json::from_str(&json).unwrap();
            assert_eq!(size, back);
        }
    }

    #[test]
    fn session_config_json_round_trip() {
        let c = SessionConfig::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: SessionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.chunk_duration_secs, c.chunk_duration_secs);
        assert_eq!(back.model_size, c.model_size);
        assert_eq!(back.language, c.language);
        assert_eq!(back.silence_threshold_rms, c.silence_threshold_rms);
    }
}
