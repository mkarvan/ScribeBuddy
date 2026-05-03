use crate::{SessionConfig, Speaker, TranscriptSegment};
use super::model::ModelManager;
use crate::audio::capture::{is_silence, TARGET_SAMPLE_RATE};
use anyhow::{Context, Result};
use chrono::Duration;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct WhisperEngine {
    context: whisper_rs::WhisperContext,
    full_params: whisper_rs::FullParams<'static, 'static>,
    silence_threshold: f32,
    chunk_duration_samples: usize,
    running: Arc<AtomicBool>,
}

impl WhisperEngine {
    pub fn new(config: &SessionConfig) -> Result<(Self, ModelManager)> {
        let model_mgr = ModelManager::new()?;
        let model_path = model_mgr.model_path(&config.model_size);

        if !model_mgr.is_model_available(&config.model_size) {
            anyhow::bail!(
                "Model not found at {:?}. Download it first.",
                model_path
            );
        }

        let context = whisper_rs::WhisperContext::new_with_params(
            &model_path,
            whisper_rs::WhisperContextParameters::default(),
        )
        .context("Failed to load Whisper model")?;

        let params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });

        let chunk_duration_samples =
            (config.chunk_duration_secs * TARGET_SAMPLE_RATE as f32) as usize;

        Ok((
            Self {
                context,
                full_params: params,
                silence_threshold: config.silence_threshold_rms,
                chunk_duration_samples,
                running: Arc::new(AtomicBool::new(false)),
            },
            model_mgr,
        ))
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn process_chunk(
        &mut self,
        samples: &[f32],
        speaker: Speaker,
        start_offset: Duration,
        segment_tx: &Sender<TranscriptSegment>,
    ) -> Result<()> {
        if samples.len() < self.chunk_duration_samples / 2 {
            return Ok(());
        }

        if is_silence(samples, self.silence_threshold) {
            return Ok(());
        }

        let chunk: Vec<f32> = if samples.len() > self.chunk_duration_samples {
            samples[..self.chunk_duration_samples].to_vec()
        } else {
            samples.to_vec()
        };

        let num_samples = chunk.len();
        let mut state = self
            .context
            .create_state()
            .context("Failed to create Whisper state")?;

        state
            .full(self.full_params.clone(), &chunk)
            .context("Whisper inference failed")?;

        let num_segments = state.full_n_segments();
        let mut text_parts: Vec<String> = Vec::new();

        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                let text = segment.to_str_lossy().unwrap_or_default();
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    text_parts.push(trimmed);
                }
            }
        }

        if !text_parts.is_empty() {
            let text = text_parts.join(" ");
            let end_offset = start_offset + Duration::milliseconds(num_samples as i64 / 16);

            let segment = TranscriptSegment::new(speaker, text, start_offset, end_offset);

            let _ = segment_tx.try_send(segment);
        }

        Ok(())
    }

    pub fn chunk_duration_secs(&self) -> f32 {
        self.chunk_duration_samples as f32 / TARGET_SAMPLE_RATE as f32
    }
}

unsafe impl Send for WhisperEngine {}
unsafe impl Sync for WhisperEngine {}
