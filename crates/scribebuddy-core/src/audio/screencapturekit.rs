use super::capture::{AudioBuffer as AudioBuf, AudioConsumer, AudioProducer, AudioRingBuffer, AudioSource};
use anyhow::Result;
use core_foundation::error::CFError;
use core_media_rs::cm_sample_buffer::CMSampleBuffer;
use parking_lot::Mutex;
use ringbuf::traits::{Producer, Split};
use screencapturekit::{
    shareable_content::SCShareableContent,
    stream::{
        configuration::SCStreamConfiguration,
        content_filter::SCContentFilter,
        output_trait::SCStreamOutputTrait,
        output_type::SCStreamOutputType,
        SCStream,
    },
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// SCK native rate; processor will resample to 16kHz
const SCK_SAMPLE_RATE: u32 = 48000;

// --- Audio output handler ---

struct AudioCaptureOutput {
    producer: Arc<Mutex<AudioProducer>>,
}

impl SCStreamOutputTrait for AudioCaptureOutput {
    fn did_output_sample_buffer(
        &self,
        sample_buffer: CMSampleBuffer,
        of_type: SCStreamOutputType,
    ) {
        if of_type != SCStreamOutputType::Audio {
            return;
        }

        let retained = match sample_buffer.get_audio_buffer_list() {
            Ok(r) => r,
            Err(e) => {
                log::warn!("SCK get_audio_buffer_list failed: {:?}", e);
                return;
            }
        };

        // Handle both interleaved (1 buf, N ch) and non-interleaved (N bufs, 1 ch each)
        let mut channel_data: Vec<Vec<f32>> = Vec::new();
        for audio_buf in retained.buffers() {
            let bytes = audio_buf.data();
            let n_samples = bytes.len() / std::mem::size_of::<f32>();
            let n_ch = audio_buf.number_channels as usize;
            if n_ch == 0 || n_samples == 0 {
                continue;
            }

            // SAFETY: SCK guarantees f32 PCM at proper alignment.
            let samples: &[f32] = unsafe {
                std::slice::from_raw_parts(bytes.as_ptr() as *const f32, n_samples)
            };

            let frames = n_samples / n_ch;
            for c in 0..n_ch {
                let ch: Vec<f32> = (0..frames).map(|f| samples[f * n_ch + c]).collect();
                channel_data.push(ch);
            }
        }

        if channel_data.is_empty() {
            return;
        }

        let frames = channel_data[0].len();
        let n_ch = channel_data.len() as f32;
        let mono: Vec<f32> = (0..frames)
            .map(|f| channel_data.iter().map(|ch| ch[f]).sum::<f32>() / n_ch)
            .collect();

        let buf = AudioBuf {
            data: mono,
            timestamp: std::time::Instant::now(),
        };

        if let Some(mut prod) = self.producer.try_lock() {
            let _ = prod.try_push(buf);
        }
    }
}

// --- ScreenCaptureKitSource ---

pub struct ScreenCaptureKitSource {
    name: String,
    bundle_id: Option<String>,
    running: Arc<AtomicBool>,
    consumer: Option<AudioConsumer>,
    active_stream: Option<SCStream>,
}

impl ScreenCaptureKitSource {
    pub fn new(name: &str, bundle_id: Option<String>) -> Self {
        Self {
            name: name.to_string(),
            bundle_id,
            running: Arc::new(AtomicBool::new(false)),
            consumer: None,
            active_stream: None,
        }
    }

    /// Returns running applications visible to SCK.
    /// Falls back to a static list when Screen Recording permission hasn't been granted yet.
    pub fn enumerate_running_apps() -> Vec<crate::RunningApp> {
        match SCShareableContent::get() {
            Ok(content) => {
                let apps = content.applications();
                if apps.is_empty() {
                    return Self::static_fallback();
                }
                apps.into_iter()
                    .map(|a| crate::RunningApp {
                        bundle_id: a.bundle_identifier(),
                        name: a.application_name(),
                    })
                    .collect()
            }
            Err(e) => {
                log::warn!("SCK enumerate_running_apps failed (permission?): {:?}", e);
                Self::static_fallback()
            }
        }
    }

    fn static_fallback() -> Vec<crate::RunningApp> {
        vec![
            ("us.zoom.xos", "Zoom"),
            ("com.microsoft.teams2", "Microsoft Teams"),
            ("com.google.Chrome", "Google Chrome"),
            ("org.mozilla.firefox", "Firefox"),
            ("com.apple.Safari", "Safari"),
            ("com.tinyspeck.slackmacgap", "Slack"),
            ("com.hnc.Discord", "Discord"),
            ("com.brave.Browser", "Brave Browser"),
            ("com.microsoft.edgemac", "Microsoft Edge"),
            ("company.thebrowser.Browser", "Arc Browser"),
        ]
        .into_iter()
        .map(|(b, n)| crate::RunningApp {
            bundle_id: b.to_string(),
            name: n.to_string(),
        })
        .collect()
    }
}

impl AudioSource for ScreenCaptureKitSource {
    fn start(&mut self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        let ring = AudioRingBuffer::new(128);
        let (prod, cons) = ring.split();
        self.consumer = Some(cons);
        let producer = Arc::new(Mutex::new(prod));

        // CFError is not Send+Sync, so convert to string before lifting into anyhow::Error
        let cfe = |e: CFError| anyhow::anyhow!("{}", e);
        let config = SCStreamConfiguration::new()
            .set_captures_audio(true).map_err(&cfe)?
            .set_channel_count(2u8).map_err(&cfe)?
            .set_sample_rate(SCK_SAMPLE_RATE).map_err(&cfe)?
            // Minimal video surface — SCK requires one even for audio-only capture
            .set_width(100u32).map_err(&cfe)?
            .set_height(100u32).map_err(&cfe)?;

        let content = SCShareableContent::get()
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let display = content
            .displays()
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No display available for SCK filter"))?;

        let filter = if let Some(ref bid) = self.bundle_id {
            let apps = content.applications();
            match apps.iter().find(|a| a.bundle_identifier() == *bid) {
                Some(app) => SCContentFilter::new()
                    .with_display_including_application_excepting_windows(&display, &[app], &[]),
                None => {
                    log::warn!(
                        "SCK: app '{}' not running, falling back to all system audio",
                        bid
                    );
                    SCContentFilter::new().with_display_excluding_windows(&display, &[])
                }
            }
        } else {
            SCContentFilter::new().with_display_excluding_windows(&display, &[])
        };

        let output = AudioCaptureOutput { producer };
        let mut stream = SCStream::new(&filter, &config);
        stream.add_output_handler(output, SCStreamOutputType::Audio);
        stream
            .start_capture()
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        self.active_stream = Some(stream);
        log::info!("SCK audio capture started for '{}'", self.name);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        if let Some(stream) = self.active_stream.take() {
            if let Err(e) = stream.stop_capture() {
                log::warn!("SCK stop_capture error: {:?}", e);
            }
        }
        log::info!("SCK audio capture stopped for '{}'", self.name);
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        SCK_SAMPLE_RATE
    }

    fn channels(&self) -> u16 {
        1 // mono after downmix in the output handler
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn take_consumer(&mut self) -> Option<AudioConsumer> {
        self.consumer.take()
    }
}
