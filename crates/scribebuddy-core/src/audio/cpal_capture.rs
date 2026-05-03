use super::capture::{AudioBuffer, AudioConsumer, AudioProducer, AudioRingBuffer, AudioSource};
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::{Producer, Split};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct CpalAudioSource {
    name: String,
    running: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
    consumer: Option<AudioConsumer>,
    capture_handle: Option<std::thread::JoinHandle<()>>,
}

impl CpalAudioSource {
    pub fn new(name: &str, sample_rate: u32, channels: u16) -> Self {
        Self {
            name: name.to_string(),
            running: Arc::new(AtomicBool::new(false)),
            sample_rate,
            channels,
            consumer: None,
            capture_handle: None,
        }
    }

    pub fn enumerate_input_devices() -> Vec<String> {
        match cpal::default_host().input_devices() {
            Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
            Err(_) => vec![],
        }
    }

    pub fn default_input_device() -> Option<String> {
        cpal::default_host()
            .default_input_device()
            .and_then(|d| d.name().ok())
    }

    fn spawn_capture_thread(
        &mut self,
        producer: AudioProducer,
        rate: u32,
        channels: u16,
    ) -> Result<()> {
        let running = self.running.clone();
        let running_loop = self.running.clone();
        let name = self.name.clone();

        // Use a one-shot channel so start() blocks until the stream either opens or fails.
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

        self.capture_handle = Some(std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err("No microphone device found".into()));
                    return;
                }
            };

            let config = cpal::StreamConfig {
                channels: channels as cpal::ChannelCount,
                sample_rate: cpal::SampleRate(rate),
                buffer_size: cpal::BufferSize::Default,
            };

            let ch = channels as usize;
            let mut prod = producer;
            let running_cb = running.clone();

            let stream = match device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !running_cb.load(Ordering::SeqCst) {
                        return;
                    }
                    // Mix down to mono
                    let mono: Vec<f32> = if ch == 1 {
                        data.to_vec()
                    } else {
                        let frames = data.len() / ch;
                        (0..frames)
                            .map(|f| data[f * ch..(f + 1) * ch].iter().sum::<f32>() / ch as f32)
                            .collect()
                    };
                    let _ = prod.try_push(AudioBuffer {
                        data: mono,
                        timestamp: std::time::Instant::now(),
                    });
                },
                |err| log::error!("[cpal] stream error: {}", err),
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    let msg = format!(
                        "Microphone access failed: {}. \
                         Grant microphone permission in System Settings → Privacy & Security → Microphone.",
                        e
                    );
                    let _ = ready_tx.send(Err(msg));
                    return;
                }
            };

            if let Err(e) = stream.play() {
                let _ = ready_tx.send(Err(format!("Failed to start mic stream: {}", e)));
                return;
            }

            log::info!("[cpal] '{}' running at {}Hz {}ch", name, rate, channels);
            let _ = ready_tx.send(Ok(()));

            while running_loop.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            drop(stream);
        }));

        // Wait up to 3 s for the stream to open (permission prompt can add a brief delay)
        match ready_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(msg)) => Err(anyhow::anyhow!("{}", msg)),
            Err(_) => Err(anyhow::anyhow!("Microphone startup timed out")),
        }
    }
}

impl AudioSource for CpalAudioSource {
    fn start(&mut self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        // Query the device's native config so we never request an unsupported rate/channel count.
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No microphone found"))?;

        let supported = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("Mic config query failed: {}", e))?;

        // Override stored values with what the hardware actually reports
        self.sample_rate = supported.sample_rate().0;
        self.channels = supported.channels();

        log::info!(
            "[cpal] mic '{}': device='{}', {}Hz, {}ch",
            self.name,
            device.name().unwrap_or_default(),
            self.sample_rate,
            self.channels
        );

        let ring = AudioRingBuffer::new(64);
        let (prod, cons) = ring.split();
        self.consumer = Some(cons);

        self.spawn_capture_thread(prod, self.sample_rate, self.channels)
    }

    fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }

        log::info!("[cpal] capture stopped for '{}'", self.name);
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn take_consumer(&mut self) -> Option<AudioConsumer> {
        self.consumer.take()
    }
}
