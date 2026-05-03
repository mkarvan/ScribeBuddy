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
        let multilingual = config.is_multilingual();
        let model_path = model_mgr.model_path(&config.model_size, multilingual);

        if !model_mgr.is_model_available(&config.model_size, multilingual) {
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

        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });

        // Disable timestamp tokens — we track timing ourselves via chunk offsets.
        // This prevents the "single timestamp ending - skip entire chunk" Whisper skip.
        params.set_no_timestamps(true);
        params.set_single_segment(true);

        // Suppress C-library console noise
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Configure language: "auto"/empty → auto-detect; specific code → force language
        // Box::leak gives a &'static str so it satisfies FullParams<'static, 'static>.
        // One small string per engine creation is an acceptable trade-off.
        let lang = config.language.trim().to_string();
        if lang.is_empty() || lang == "auto" {
            params.set_detect_language(true);
        } else {
            let lang_static: &'static str = Box::leak(lang.into_boxed_str());
            params.set_language(Some(lang_static));
        }

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

        let rms_in = rms(samples);
        log::debug!(
            "[whisper] {:?} input: {} samples @ 16kHz, rms={:.4}",
            speaker, samples.len(), rms_in
        );

        let num_segments = state.full_n_segments();
        let mut text_parts: Vec<String> = Vec::new();

        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                let text = segment.to_str_lossy().unwrap_or_default();
                let trimmed = text.trim().to_string();
                log::debug!("[whisper] raw segment {}: {:?}", i, trimmed);
                // Drop Whisper non-speech annotation tokens: [MUSIC], (growling), etc.
                if !trimmed.is_empty() && !is_annotation_token(&trimmed) {
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

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() { return 0.0; }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Returns true for Whisper non-speech annotation tokens.
/// Covers [MUSIC], [BLANK_AUDIO], (growling), *burps*, ♪text♪ etc.
fn is_annotation_token(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with('[') && t.ends_with(']'))
        || (t.starts_with('(') && t.ends_with(')'))
        || (t.starts_with('*') && t.ends_with('*') && t.len() > 1)
        || (t.starts_with('♪') && t.ends_with('♪'))
        || (t.starts_with('♫') && t.ends_with('♫'))
}
