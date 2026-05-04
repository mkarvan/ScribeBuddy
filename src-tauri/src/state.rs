use crossbeam_channel::Sender;
use parking_lot::Mutex;
use scribebuddy_core::session::manager::SessionManager;
use scribebuddy_core::{SessionConfig, TranscriptSegment};
use std::path::{Path, PathBuf};

pub struct AppState {
    pub config: Mutex<SessionConfig>,
    pub config_path: PathBuf,
    pub session: Mutex<SessionManager>,
    pub accumulated: std::sync::Arc<parking_lot::RwLock<Vec<TranscriptSegment>>>,
    pub model_downloading: std::sync::Arc<parking_lot::RwLock<bool>>,
    /// Held so the segment/error listener threads outlive pause/resume cycles.
    pub segment_tx: Mutex<Option<Sender<TranscriptSegment>>>,
    pub error_tx: Mutex<Option<Sender<String>>>,
}

impl AppState {
    pub fn new(config: SessionConfig, config_path: PathBuf) -> Self {
        let session = SessionManager::new(config.clone());
        Self {
            config: Mutex::new(config),
            config_path,
            session: Mutex::new(session),
            accumulated: std::sync::Arc::new(parking_lot::RwLock::new(Vec::new())),
            model_downloading: std::sync::Arc::new(parking_lot::RwLock::new(false)),
            segment_tx: Mutex::new(None),
            error_tx: Mutex::new(None),
        }
    }

    /// Load config from disk; returns default on any error (missing file, bad JSON, etc.).
    /// Migrates any field values that used old defaults.
    pub fn load_config(path: &Path) -> SessionConfig {
        let mut config: SessionConfig = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Migrate: old default was 0.01, which is too high for many mics (typical
        // quiet-room mic RMS is 0.003–0.008). Reset to new default if unchanged.
        if (config.silence_threshold_rms - 0.01).abs() < f32::EPSILON {
            config.silence_threshold_rms = SessionConfig::default().silence_threshold_rms;
        }
        config
    }

    /// Persist current config to disk. Called after every setter command.
    pub fn save_config(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&*self.config.lock())
            .map_err(|e| format!("Config serialize error: {}", e))?;
        atomic_write(&self.config_path, json.as_bytes())
            .map_err(|e| format!("Config write error: {}", e))
    }

    /// Standalone save used by the background thread in stop_session so
    /// serializing a large transcript does not block the UI.
    pub fn write_transcript(config_path: PathBuf, segments: Vec<TranscriptSegment>) -> Result<(), String> {
        if segments.is_empty() {
            return Ok(());
        }

        let sessions_dir = config_path
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
        atomic_write(&path, json.as_bytes())
            .map_err(|e| format!("Transcript write error: {}", e))?;

        log::info!("Transcript auto-saved to {:?}", path);
        Ok(())
    }

    pub fn get_segments(&self) -> Vec<TranscriptSegment> {
        self.accumulated.read().clone()
    }

    pub fn clear_segments(&self) {
        self.accumulated.write().clear();
    }
}

/// Write-to-temp-then-rename so a crash mid-write never corrupts the target file.
fn atomic_write(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}
