use crate::audio::capture::AudioConsumer;
use crate::transcription::engine::WhisperEngine;
use crate::{SessionConfig, Speaker, TranscriptSegment};
use chrono::Duration;
use crossbeam_channel::Sender;
use ringbuf::traits::Consumer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct AudioProcessor {
    config: SessionConfig,
    running: Arc<AtomicBool>,
    segment_tx: Sender<TranscriptSegment>,
    error_tx: Sender<String>,
}

impl AudioProcessor {
    pub fn new(
        config: SessionConfig,
        running: Arc<AtomicBool>,
        segment_tx: Sender<TranscriptSegment>,
        error_tx: Sender<String>,
    ) -> Self {
        Self {
            config,
            running,
            segment_tx,
            error_tx,
        }
    }

    pub fn run(
        self,
        mut you_consumer: AudioConsumer,
        mut remote_consumer: AudioConsumer,
        you_source_rate: u32,
        remote_source_rate: u32,
    ) {
        let you_chunk_target =
            (self.config.chunk_duration_secs * you_source_rate as f32) as usize;
        let remote_chunk_target =
            (self.config.chunk_duration_secs * remote_source_rate as f32) as usize;

        let mut you_buffer: Vec<f32> = Vec::with_capacity(you_chunk_target * 2);
        let mut remote_buffer: Vec<f32> = Vec::with_capacity(remote_chunk_target * 2);

        let (mut engine, _model_mgr) = match WhisperEngine::new(&self.config) {
            Ok(e) => e,
            Err(err) => {
                let msg = format!("Failed to load Whisper model: {}", err);
                log::error!("{}", msg);
                let _ = self.error_tx.send(msg);
                return;
            }
        };

        // Signal that processor is ready
        log::info!("Audio processor running, chunk targets: you={} remote={}",
            you_chunk_target, remote_chunk_target);

        let chunk_dur = Duration::milliseconds((self.config.chunk_duration_secs * 1000.0) as i64);
        let mut you_offset = Duration::zero();
        let mut remote_offset = Duration::zero();

        while self.running.load(Ordering::SeqCst) {
            let mut processed = false;

            while let Some(buf) = you_consumer.try_pop() {
                you_buffer.extend_from_slice(&buf.data);
            }

            while let Some(buf) = remote_consumer.try_pop() {
                remote_buffer.extend_from_slice(&buf.data);
            }

            if you_buffer.len() >= you_chunk_target {
                let chunk: Vec<f32> = you_buffer.drain(..you_chunk_target).collect();
                if let Err(e) = engine.process_chunk(&chunk, Speaker::You, you_offset, &self.segment_tx) {
                    log::error!("You Whisper error: {}", e);
                    let _ = self.error_tx.send(format!("Whisper error (You): {}", e));
                }
                you_offset = you_offset + chunk_dur;
                processed = true;
            }

            if remote_buffer.len() >= remote_chunk_target {
                let chunk: Vec<f32> = remote_buffer.drain(..remote_chunk_target).collect();
                if let Err(e) = engine.process_chunk(&chunk, Speaker::Remote, remote_offset, &self.segment_tx) {
                    log::error!("Remote Whisper error: {}", e);
                    let _ = self.error_tx.send(format!("Whisper error (Remote): {}", e));
                }
                remote_offset = remote_offset + chunk_dur;
                processed = true;
            }

            if !processed {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        log::info!("Audio processor stopped");
    }
}
