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
    pub start_time: chrono::Duration,
    pub end_time: chrono::Duration,
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
}

impl ModelSize {
    pub fn filename(&self) -> &str {
        match self {
            ModelSize::Tiny => "ggml-tiny.en.bin",
            ModelSize::Base => "ggml-base.en.bin",
            ModelSize::Small => "ggml-small.en.bin",
            ModelSize::Medium => "ggml-medium.en.bin",
        }
    }

    pub fn download_url(&self) -> &str {
        match self {
            ModelSize::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
            ModelSize::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
            ModelSize::Small => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
            ModelSize::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin",
        }
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
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            target_app_bundle_id: None,
            chunk_duration_secs: 3.0,
            model_size: ModelSize::default(),
            use_screencapturekit: true,
            silence_threshold_rms: 0.01,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningApp {
    pub bundle_id: String,
    pub name: String,
}
