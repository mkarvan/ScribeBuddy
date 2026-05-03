use parking_lot::Mutex;
use scribebuddy_core::audio::capture::AudioSource;
use scribebuddy_core::{SessionConfig, SessionState, TranscriptSegment};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct AppState {
    pub config: Mutex<SessionConfig>,
    pub config_path: PathBuf,
    pub session_state: Arc<parking_lot::RwLock<SessionState>>,
    pub running: Arc<AtomicBool>,
    pub accumulated: Arc<parking_lot::RwLock<Vec<TranscriptSegment>>>,
    pub you_source: Mutex<Option<Box<dyn AudioSource>>>,
    pub remote_source: Mutex<Option<Box<dyn AudioSource>>>,
    pub processor_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub model_downloading: Arc<parking_lot::RwLock<bool>>,
}

impl AppState {
    pub fn new(config: SessionConfig, config_path: PathBuf) -> Self {
        Self {
            config: Mutex::new(config),
            config_path,
            session_state: Arc::new(parking_lot::RwLock::new(SessionState::Idle)),
            running: Arc::new(AtomicBool::new(false)),
            accumulated: Arc::new(parking_lot::RwLock::new(Vec::new())),
            you_source: Mutex::new(None),
            remote_source: Mutex::new(None),
            processor_handle: Mutex::new(None),
            model_downloading: Arc::new(parking_lot::RwLock::new(false)),
        }
    }

    /// Load config from disk; returns default on any error (missing file, bad JSON, etc.)
    pub fn load_config(path: &Path) -> SessionConfig {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist current config to disk. Called after every setter command.
    pub fn save_config(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&*self.config.lock())
            .map_err(|e| format!("Config serialize error: {}", e))?;
        std::fs::write(&self.config_path, json)
            .map_err(|e| format!("Config write error: {}", e))
    }

    /// Auto-save accumulated transcript segments as JSON when a session stops.
    /// Errors are logged by the caller but do not fail the stop command.
    pub fn save_session_transcript(&self) -> Result<(), String> {
        let segments = self.accumulated.read().clone();
        if segments.is_empty() {
            return Ok(());
        }

        let sessions_dir = self
            .config_path
            .parent()
            .ok_or("config_path has no parent directory")?
            .join("sessions");

        std::fs::create_dir_all(&sessions_dir)
            .map_err(|e| format!("Cannot create sessions dir: {}", e))?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = sessions_dir.join(format!("session-{}.json", ts));

        let json = serde_json::to_string_pretty(&segments)
            .map_err(|e| format!("Transcript serialize error: {}", e))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("Transcript write error: {}", e))?;

        log::info!("Transcript auto-saved to {:?}", path);
        Ok(())
    }

    pub fn push_segment(&self, segment: TranscriptSegment) {
        self.accumulated.write().push(segment);
    }

    pub fn get_segments(&self) -> Vec<TranscriptSegment> {
        self.accumulated.read().clone()
    }

    pub fn clear_segments(&self) {
        self.accumulated.write().clear();
    }
}
