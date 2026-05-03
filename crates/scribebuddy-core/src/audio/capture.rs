use anyhow::Result;
use ringbuf::HeapCons;
use ringbuf::HeapProd;
use ringbuf::HeapRb;

pub const TARGET_SAMPLE_RATE: u32 = 16000;
pub const CHANNELS: u16 = 1;

pub struct AudioBuffer {
    pub data: Vec<f32>,
    pub timestamp: std::time::Instant,
}

pub type AudioRingBuffer = HeapRb<AudioBuffer>;
pub type AudioProducer = HeapProd<AudioBuffer>;
pub type AudioConsumer = HeapCons<AudioBuffer>;

pub trait AudioSource: Send {
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn name(&self) -> &str;
    fn take_consumer(&mut self) -> Option<AudioConsumer>;
}

pub fn rms_energy(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

pub fn is_silence(samples: &[f32], threshold: f32) -> bool {
    rms_energy(samples) < threshold
}
