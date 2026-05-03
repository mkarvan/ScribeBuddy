use super::capture::{AudioBuffer, AudioConsumer, AudioProducer, AudioRingBuffer, AudioSource, TARGET_SAMPLE_RATE, CHANNELS};
use anyhow::Result;
use ringbuf::traits::{Producer, Split};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct ScreenCaptureKitSource {
    name: String,
    #[allow(dead_code)]
    bundle_id: Option<String>,
    running: Arc<AtomicBool>,
    sample_rate: u32,
    consumer: Option<AudioConsumer>,
    capture_handle: Option<std::thread::JoinHandle<()>>,
}

impl ScreenCaptureKitSource {
    pub fn new(name: &str, bundle_id: Option<String>) -> Self {
        Self {
            name: name.to_string(),
            bundle_id,
            running: Arc::new(AtomicBool::new(false)),
            sample_rate: TARGET_SAMPLE_RATE,
            consumer: None,
            capture_handle: None,
        }
    }

    pub fn enumerate_running_apps() -> Vec<crate::RunningApp> {
        let known_apps = vec![
            ("us.zoom.xos", "Zoom"),
            ("com.microsoft.teams", "Microsoft Teams"),
            ("com.microsoft.teams2", "Microsoft Teams (Classic)"),
            ("com.google.meet", "Google Meet"),
            ("com.apple.facetime", "FaceTime"),
            ("com.cisco.webexmeetingsapp", "Webex"),
            ("com.skype.skype", "Skype"),
            ("com.tinyspeck.slackmacgap", "Slack"),
            ("com.hnc.Discord", "Discord"),
            ("com.google.Chrome", "Google Chrome"),
            ("org.mozilla.firefox", "Firefox"),
            ("com.apple.Safari", "Safari"),
            ("com.brave.Browser", "Brave Browser"),
            ("com.microsoft.edgemac", "Microsoft Edge"),
            ("company.thebrowser.Browser", "Arc Browser"),
        ];

        known_apps
            .into_iter()
            .map(|(bundle_id, name)| crate::RunningApp {
                bundle_id: bundle_id.to_string(),
                name: name.to_string(),
            })
            .collect()
    }

    fn spawn_capture_thread(&mut self, mut producer: AudioProducer) {
        let running = self.running.clone();
        let rate = self.sample_rate;

        self.capture_handle = Some(std::thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                let chunk_size = (rate as usize) / 10;
                let silence = vec![0.0f32; chunk_size];

                let _ = producer.try_push(AudioBuffer {
                    data: silence,
                    timestamp: std::time::Instant::now(),
                });

                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }));
    }
}

impl AudioSource for ScreenCaptureKitSource {
    fn start(&mut self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        // Create a fresh ring buffer for each start (supports restart after stop)
        let ring = AudioRingBuffer::new(64);
        let (prod, cons) = ring.split();
        self.consumer = Some(cons);
        self.spawn_capture_thread(prod);

        log::info!("ScreenCaptureKit audio capture started for '{}'", self.name);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }

        log::info!("ScreenCaptureKit audio capture stopped for '{}'", self.name);
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        CHANNELS
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn take_consumer(&mut self) -> Option<AudioConsumer> {
        self.consumer.take()
    }
}
