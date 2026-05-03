use crate::audio::capture::AudioSource;
use crate::audio::processor::AudioProcessor;
use crate::{SessionConfig, SessionState, TranscriptSegment};
use anyhow::Result;
use crossbeam_channel::{self, Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct SessionManager {
    state: Arc<parking_lot::RwLock<SessionState>>,
    config: SessionConfig,
    segment_tx: Sender<TranscriptSegment>,
    segment_rx: Option<Receiver<TranscriptSegment>>,
    running: Arc<AtomicBool>,
    processor_handle: Option<std::thread::JoinHandle<()>>,
    you_source: Option<Box<dyn AudioSource>>,
    remote_source: Option<Box<dyn AudioSource>>,
}

impl SessionManager {
    pub fn new(config: SessionConfig) -> Result<Self> {
        let (segment_tx, segment_rx) = crossbeam_channel::bounded(256);
        Ok(Self {
            state: Arc::new(parking_lot::RwLock::new(SessionState::Idle)),
            config,
            segment_tx,
            segment_rx: Some(segment_rx),
            running: Arc::new(AtomicBool::new(false)),
            processor_handle: None,
            you_source: None,
            remote_source: None,
        })
    }

    pub fn state(&self) -> SessionState {
        self.state.read().clone()
    }

    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    pub fn segment_receiver(&self) -> Option<Receiver<TranscriptSegment>> {
        self.segment_rx.clone()
    }

    pub fn segment_sender(&self) -> Sender<TranscriptSegment> {
        self.segment_tx.clone()
    }

    pub fn set_you_source(&mut self, source: Box<dyn AudioSource>) {
        self.you_source = Some(source);
    }

    pub fn set_remote_source(&mut self, source: Box<dyn AudioSource>) {
        self.remote_source = Some(source);
    }

    pub fn start(&mut self) -> Result<()> {
        let current = self.state.read().clone();
        if current != SessionState::Idle && current != SessionState::Stopped {
            anyhow::bail!("Cannot start: session is {}", current);
        }

        self.running.store(true, Ordering::SeqCst);

        // Start "You" audio source (microphone)
        let you_consumer = if let Some(ref mut you) = self.you_source {
            you.start()?;
            you.take_consumer()
        } else {
            None
        };

        // Start "Remote" audio source (system audio)
        let remote_consumer = if let Some(ref mut remote) = self.remote_source {
            remote.start()?;
            remote.take_consumer()
        } else {
            None
        };

        // Get sample rates for processor
        let you_rate = self.you_source.as_ref().map(|s| s.sample_rate()).unwrap_or(16000);
        let remote_rate = self.remote_source.as_ref().map(|s| s.sample_rate()).unwrap_or(16000);

        // Start audio processor thread
        let (error_tx, _error_rx) = crossbeam_channel::bounded(16);

        if let (Some(you_cons), Some(remote_cons)) = (you_consumer, remote_consumer) {
            let processor = AudioProcessor::new(
                self.config.clone(),
                self.running.clone(),
                self.segment_tx.clone(),
                error_tx,
            );

            self.processor_handle = Some(std::thread::spawn(move || {
                processor.run(you_cons, remote_cons, you_rate, remote_rate);
            }));
        } else {
            let _processor = AudioProcessor::new(
                self.config.clone(),
                self.running.clone(),
                self.segment_tx.clone(),
                error_tx,
            );

            self.processor_handle = Some(std::thread::spawn(move || {
                log::warn!("No audio sources configured, running in mock mode");
            }));
        }

        *self.state.write() = SessionState::Recording;

        log::info!("Session started");
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        let current = self.state.read().clone();
        if current != SessionState::Recording {
            anyhow::bail!("Cannot pause: session is {}", current);
        }

        self.running.store(false, Ordering::SeqCst);

        // Pause audio sources
        if let Some(ref mut you) = self.you_source {
            let _ = you.stop();
        }
        if let Some(ref mut remote) = self.remote_source {
            let _ = remote.stop();
        }

        *self.state.write() = SessionState::Paused;

        log::info!("Session paused");
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        let current = self.state.read().clone();
        if current != SessionState::Paused {
            anyhow::bail!("Cannot resume: session is {}", current);
        }

        self.running.store(true, Ordering::SeqCst);

        // Resume audio sources
        if let Some(ref mut you) = self.you_source {
            you.start()?;
        }
        if let Some(ref mut remote) = self.remote_source {
            remote.start()?;
        }

        *self.state.write() = SessionState::Recording;

        log::info!("Session resumed");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        let current = self.state.read().clone();
        if current != SessionState::Recording && current != SessionState::Paused {
            anyhow::bail!("Cannot stop: session is {}", current);
        }

        self.running.store(false, Ordering::SeqCst);

        // Stop audio sources
        if let Some(ref mut you) = self.you_source {
            let _ = you.stop();
        }
        if let Some(ref mut remote) = self.remote_source {
            let _ = remote.stop();
        }

        // Wait for processor to finish
        if let Some(handle) = self.processor_handle.take() {
            let _ = handle.join();
        }

        *self.state.write() = SessionState::Stopped;

        log::info!("Session stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let config = SessionConfig::default();
        let session = SessionManager::new(config).unwrap();
        assert_eq!(session.state(), SessionState::Idle);
    }

    #[test]
    fn test_pause_resume_stop_without_start() {
        let config = SessionConfig::default();
        let mut session = SessionManager::new(config).unwrap();
        assert_eq!(session.state(), SessionState::Idle);

        assert!(session.pause().is_err());
        assert!(session.resume().is_err());
        assert!(session.stop().is_err());
    }
}
