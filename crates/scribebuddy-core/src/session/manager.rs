use crate::audio::capture::AudioSource;
use crate::audio::processor::AudioProcessor;
use crate::{SessionConfig, SessionState, TranscriptSegment};
use anyhow::Result;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct SessionManager {
    state: SessionState,
    config: SessionConfig,
    running: Arc<AtomicBool>,
    processor_handle: Option<std::thread::JoinHandle<()>>,
    you_source: Option<Box<dyn AudioSource>>,
    remote_source: Option<Box<dyn AudioSource>>,
}

impl SessionManager {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            state: SessionState::Idle,
            config,
            running: Arc::new(AtomicBool::new(false)),
            processor_handle: None,
            you_source: None,
            remote_source: None,
        }
    }

    pub fn state(&self) -> SessionState {
        self.state.clone()
    }

    pub fn running(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    pub fn set_you_source(&mut self, source: Box<dyn AudioSource>) {
        self.you_source = Some(source);
    }

    pub fn set_remote_source(&mut self, source: Box<dyn AudioSource>) {
        self.remote_source = Some(source);
    }

    pub fn start(
        &mut self,
        segment_tx: Sender<TranscriptSegment>,
        error_tx: Sender<String>,
    ) -> Result<()> {
        if self.state != SessionState::Idle && self.state != SessionState::Stopped {
            anyhow::bail!("Cannot start: session is {}", self.state);
        }

        self.running.store(true, Ordering::SeqCst);

        let you_source = self.you_source.as_mut().ok_or_else(|| anyhow::anyhow!("No mic source configured"))?;
        you_source.start()?;
        let you_consumer = you_source.take_consumer().ok_or_else(|| anyhow::anyhow!("No mic consumer"))?;
        let you_rate = you_source.sample_rate();

        let remote_source = self.remote_source.as_mut().ok_or_else(|| anyhow::anyhow!("No remote source configured"))?;
        remote_source.start()?;
        let remote_consumer = remote_source.take_consumer().ok_or_else(|| anyhow::anyhow!("No remote consumer"))?;
        let remote_rate = remote_source.sample_rate();

        let processor = AudioProcessor::new(self.config.clone(), self.running.clone(), segment_tx, error_tx);
        self.processor_handle = Some(std::thread::spawn(move || {
            processor.run(you_consumer, remote_consumer, you_rate, remote_rate);
        }));

        self.state = SessionState::Recording;
        log::info!("Session started");
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        if self.state != SessionState::Recording {
            anyhow::bail!("Cannot pause: session is {}", self.state);
        }

        self.running.store(false, Ordering::SeqCst);
        if let Some(ref mut you) = self.you_source { let _ = you.stop(); }
        if let Some(ref mut remote) = self.remote_source { let _ = remote.stop(); }
        if let Some(handle) = self.processor_handle.take() { let _ = handle.join(); }

        self.state = SessionState::Paused;
        log::info!("Session paused");
        Ok(())
    }

    pub fn resume(
        &mut self,
        segment_tx: Sender<TranscriptSegment>,
        error_tx: Sender<String>,
    ) -> Result<()> {
        if self.state != SessionState::Paused {
            anyhow::bail!("Cannot resume: session is {}", self.state);
        }

        self.running.store(true, Ordering::SeqCst);

        // Each start() creates a fresh ring buffer; take the new consumer.
        let you_source = self.you_source.as_mut().ok_or_else(|| anyhow::anyhow!("No mic source"))?;
        you_source.start()?;
        let you_consumer = you_source.take_consumer().ok_or_else(|| anyhow::anyhow!("No mic consumer after resume"))?;
        let you_rate = you_source.sample_rate();

        let remote_source = self.remote_source.as_mut().ok_or_else(|| anyhow::anyhow!("No remote source"))?;
        remote_source.start()?;
        let remote_consumer = remote_source.take_consumer().ok_or_else(|| anyhow::anyhow!("No remote consumer after resume"))?;
        let remote_rate = remote_source.sample_rate();

        let processor = AudioProcessor::new(self.config.clone(), self.running.clone(), segment_tx, error_tx);
        self.processor_handle = Some(std::thread::spawn(move || {
            processor.run(you_consumer, remote_consumer, you_rate, remote_rate);
        }));

        self.state = SessionState::Recording;
        log::info!("Session resumed");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if self.state != SessionState::Recording && self.state != SessionState::Paused {
            anyhow::bail!("Cannot stop: session is {}", self.state);
        }

        self.running.store(false, Ordering::SeqCst);
        if let Some(ref mut you) = self.you_source { let _ = you.stop(); }
        if let Some(ref mut remote) = self.remote_source { let _ = remote.stop(); }
        if let Some(handle) = self.processor_handle.take() { let _ = handle.join(); }

        self.state = SessionState::Stopped;
        log::info!("Session stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let session = SessionManager::new(SessionConfig::default());
        assert_eq!(session.state(), SessionState::Idle);
    }

    #[test]
    fn test_pause_resume_stop_without_start() {
        let mut session = SessionManager::new(SessionConfig::default());
        assert_eq!(session.state(), SessionState::Idle);

        let (tx, _rx) = crossbeam_channel::bounded(1);
        let (etx, _erx) = crossbeam_channel::bounded(1);

        assert!(session.pause().is_err());
        assert!(session.resume(tx.clone(), etx.clone()).is_err());
        assert!(session.stop().is_err());
    }
}
