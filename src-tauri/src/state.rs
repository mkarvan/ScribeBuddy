use parking_lot::Mutex;
use scribebuddy_core::audio::capture::AudioSource;
use scribebuddy_core::{SessionConfig, SessionState, TranscriptSegment};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct AppState {
    pub config: Mutex<SessionConfig>,
    pub session_state: Arc<parking_lot::RwLock<SessionState>>,
    pub running: Arc<AtomicBool>,
    pub accumulated: Arc<parking_lot::RwLock<Vec<TranscriptSegment>>>,
    pub you_source: Mutex<Option<Box<dyn AudioSource>>>,
    pub remote_source: Mutex<Option<Box<dyn AudioSource>>>,
    pub processor_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub model_downloading: Arc<parking_lot::RwLock<bool>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(SessionConfig::default()),
            session_state: Arc::new(parking_lot::RwLock::new(SessionState::Idle)),
            running: Arc::new(AtomicBool::new(false)),
            accumulated: Arc::new(parking_lot::RwLock::new(Vec::new())),
            you_source: Mutex::new(None),
            remote_source: Mutex::new(None),
            processor_handle: Mutex::new(None),
            model_downloading: Arc::new(parking_lot::RwLock::new(false)),
        }
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
