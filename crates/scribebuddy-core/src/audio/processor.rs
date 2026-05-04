use crate::audio::capture::{is_silence, AudioConsumer};
use crate::audio::resampler::AudioResampler;
use crate::transcription::{engine::WhisperEngine, Transcriber};
use crate::{SessionConfig, Speaker, TranscriptSegment};
use crossbeam_channel::Sender;
use ringbuf::traits::Consumer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const WHISPER_RATE: u32 = 16000;

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
        you_consumer: AudioConsumer,
        remote_consumer: AudioConsumer,
        you_source_rate: u32,
        remote_source_rate: u32,
    ) {
        let engine = match WhisperEngine::new(&self.config) {
            Ok(e) => e,
            Err(err) => {
                let msg = format!("Failed to load Whisper model: {}", err);
                log::error!("{}", msg);
                let _ = self.error_tx.try_send(msg);
                return;
            }
        };
        self.run_with(you_consumer, remote_consumer, you_source_rate, remote_source_rate, engine);
    }

    pub fn run_with<T: Transcriber>(
        self,
        mut you_consumer: AudioConsumer,
        mut remote_consumer: AudioConsumer,
        you_source_rate: u32,
        remote_source_rate: u32,
        mut transcriber: T,
    ) {
        let you_chunk_target =
            (self.config.chunk_duration_secs * you_source_rate as f32) as usize;
        let remote_chunk_target =
            (self.config.chunk_duration_secs * remote_source_rate as f32) as usize;

        let mut you_buffer: Vec<f32> = Vec::with_capacity(you_chunk_target * 2);
        let mut remote_buffer: Vec<f32> = Vec::with_capacity(remote_chunk_target * 2);

        let mut you_resampler = match AudioResampler::new(you_source_rate, WHISPER_RATE, you_chunk_target) {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("Failed to create resampler (you): {}", e);
                log::error!("{}", msg);
                let _ = self.error_tx.try_send(msg);
                return;
            }
        };

        let mut remote_resampler = match AudioResampler::new(remote_source_rate, WHISPER_RATE, remote_chunk_target) {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("Failed to create resampler (remote): {}", e);
                log::error!("{}", msg);
                let _ = self.error_tx.try_send(msg);
                return;
            }
        };

        log::info!(
            "Audio processor running, chunk targets: you={} remote={}",
            you_chunk_target,
            remote_chunk_target
        );

        // Debug WAV dump: capture the first 30 s of resampled remote audio (what Whisper sees)
        // to ~/Desktop/scribebuddy_whisper_input.wav. Disabled in release builds.
        #[cfg(debug_assertions)]
        let mut wav_buf: Vec<f32> = Vec::new();
        #[cfg(debug_assertions)]
        let wav_max_samples = (30.0 * WHISPER_RATE as f32) as usize;
        #[cfg(debug_assertions)]
        let mut wav_written = false;

        let chunk_secs = self.config.chunk_duration_secs.round() as i64;
        let mut you_offset_secs: i64 = 0;
        let mut remote_offset_secs: i64 = 0;
        let silence_threshold = self.config.silence_threshold_rms;

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
                let rms_you = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
                log::debug!(
                    "[proc] you chunk ready: {} samples @{}Hz, rms={:.4}",
                    chunk.len(), you_source_rate, rms_you
                );
                if !is_silence(&chunk, silence_threshold) {
                    match you_resampler.resample(&chunk) {
                        Ok(resampled) => {
                            let rms_out = (resampled.iter().map(|s| s * s).sum::<f32>() / resampled.len() as f32).sqrt();
                            log::debug!(
                                "[proc] you resampled: {} samples @16kHz, rms={:.4}",
                                resampled.len(), rms_out
                            );
                            match transcriber.transcribe(&resampled, Speaker::You, you_offset_secs) {
                                Ok(segments) => {
                                    for seg in segments {
                                        let _ = self.segment_tx.try_send(seg);
                                    }
                                }
                                Err(e) => {
                                    log::error!("You transcriber error: {}", e);
                                    let _ = self.error_tx.try_send(format!("Transcriber error (You): {}", e));
                                }
                            }
                        }
                        Err(e) => log::error!("Resample error (you): {}", e),
                    }
                } else {
                    log::debug!("[proc] you chunk silent (rms={:.4}), skipped", rms_you);
                }
                you_offset_secs += chunk_secs;
                processed = true;
            }

            if remote_buffer.len() >= remote_chunk_target {
                let chunk: Vec<f32> = remote_buffer.drain(..remote_chunk_target).collect();
                let rms_raw = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
                log::debug!(
                    "[proc] remote chunk ready: {} samples @{}Hz, rms={:.4}",
                    chunk.len(), remote_source_rate, rms_raw
                );
                if !is_silence(&chunk, silence_threshold) {
                    match remote_resampler.resample(&chunk) {
                        Ok(resampled) => {
                            let rms_out = (resampled.iter().map(|s| s * s).sum::<f32>() / resampled.len() as f32).sqrt();
                            log::debug!(
                                "[proc] remote resampled: {} samples @16kHz, rms={:.4}",
                                resampled.len(), rms_out
                            );

                            // Accumulate into WAV debug dump (debug builds only)
                            #[cfg(debug_assertions)]
                            if !wav_written && wav_buf.len() < wav_max_samples {
                                wav_buf.extend_from_slice(&resampled);
                                if wav_buf.len() >= wav_max_samples {
                                    write_debug_wav(&wav_buf, WHISPER_RATE);
                                    wav_written = true;
                                }
                            }

                            match transcriber.transcribe(&resampled, Speaker::Remote, remote_offset_secs) {
                                Ok(segments) => {
                                    for seg in segments {
                                        let _ = self.segment_tx.try_send(seg);
                                    }
                                }
                                Err(e) => {
                                    log::error!("Remote transcriber error: {}", e);
                                    let _ = self.error_tx.try_send(format!("Transcriber error (Remote): {}", e));
                                }
                            }
                        }
                        Err(e) => log::error!("Resample error (remote): {}", e),
                    }
                } else {
                    log::debug!("[proc] remote chunk silent (rms={:.4} < threshold {:.4}), skipped", rms_raw, silence_threshold);
                }
                remote_offset_secs += chunk_secs;
                processed = true;
            }

            if !processed {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        log::info!("Audio processor stopped");
    }
}

/// Writes a 32-bit float mono WAV to ~/Desktop/scribebuddy_whisper_input.wav.
/// Only compiled in debug builds — used for audio-pipeline diagnosis.
#[cfg(debug_assertions)]
fn write_debug_wav(samples: &[f32], sample_rate: u32) {
    use std::io::Write;
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let path = format!("{}/Desktop/scribebuddy_whisper_input.wav", home);

    let do_write = || -> std::io::Result<()> {
        let mut f = std::fs::File::create(&path)?;
        let data_bytes = (samples.len() * 4) as u32;

        // RIFF header
        f.write_all(b"RIFF")?;
        f.write_all(&(36 + data_bytes).to_le_bytes())?;
        f.write_all(b"WAVE")?;

        // fmt chunk — IEEE float (format tag 3)
        f.write_all(b"fmt ")?;
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&3u16.to_le_bytes())?;           // IEEE float
        f.write_all(&1u16.to_le_bytes())?;           // mono
        f.write_all(&sample_rate.to_le_bytes())?;
        f.write_all(&(sample_rate * 4).to_le_bytes())?;  // byte rate
        f.write_all(&4u16.to_le_bytes())?;           // block align
        f.write_all(&32u16.to_le_bytes())?;          // bits per sample

        // data chunk
        f.write_all(b"data")?;
        f.write_all(&data_bytes.to_le_bytes())?;
        for &s in samples {
            f.write_all(&s.to_le_bytes())?;
        }
        Ok(())
    };

    match do_write() {
        Ok(()) => log::info!("WAV debug dump written to {}", path),
        Err(e) => log::error!("WAV debug dump failed: {}", e),
    }
}
