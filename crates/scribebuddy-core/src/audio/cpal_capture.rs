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

    fn spawn_capture_thread(&mut self, mut producer: AudioProducer) {
        let running = self.running.clone();
        let running_loop = self.running.clone();
        let rate = self.sample_rate;
        let channels = self.channels;

        self.capture_handle = Some(std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    log::error!("No input device found for '{}'", "cpal source");
                    return;
                }
            };

            let config = cpal::StreamConfig {
                channels: channels as cpal::ChannelCount,
                sample_rate: cpal::SampleRate(rate),
                buffer_size: cpal::BufferSize::Default,
            };

            let running_cb = running.clone();

            let stream = match device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !running_cb.load(Ordering::SeqCst) {
                        return;
                    }
                    let buf = AudioBuffer {
                        data: data.to_vec(),
                        timestamp: std::time::Instant::now(),
                    };
                    let _ = producer.try_push(buf);
                },
                |err| {
                    log::error!("cpal audio error: {}", err);
                },
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to build input stream: {}", e);
                    return;
                }
            };

            if let Err(e) = stream.play() {
                log::error!("Failed to start audio stream: {}", e);
                return;
            }

            while running_loop.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }

            drop(stream);
        }));
    }
}

impl AudioSource for CpalAudioSource {
    fn start(&mut self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        // Create a fresh ring buffer for each start (supports restart after stop)
        let ring = AudioRingBuffer::new(64);
        let (prod, cons) = ring.split();
        self.consumer = Some(cons);
        self.spawn_capture_thread(prod);

        log::info!("cpal audio capture started for '{}'", self.name);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }

        log::info!("cpal audio capture stopped for '{}'", self.name);
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
