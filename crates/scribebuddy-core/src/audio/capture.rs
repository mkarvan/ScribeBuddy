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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_energy_empty() {
        assert_eq!(rms_energy(&[]), 0.0);
    }

    #[test]
    fn rms_energy_dc_signal() {
        // Constant signal at amplitude A → RMS should equal A
        let samples = vec![0.5f32; 1000];
        let rms = rms_energy(&samples);
        assert!((rms - 0.5).abs() < 1e-5, "rms={}", rms);
    }

    #[test]
    fn rms_energy_zero_signal() {
        let samples = vec![0.0f32; 512];
        assert_eq!(rms_energy(&samples), 0.0);
    }

    #[test]
    fn rms_energy_full_scale() {
        // Full-scale sine wave: RMS ≈ 1/√2 ≈ 0.7071
        let samples: Vec<f32> = (0..16000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();
        let rms = rms_energy(&samples);
        assert!((rms - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.001, "rms={}", rms);
    }

    #[test]
    fn is_silence_below_threshold() {
        let samples = vec![0.001f32; 512];
        assert!(is_silence(&samples, 0.01));
    }

    #[test]
    fn is_silence_above_threshold() {
        let samples = vec![0.5f32; 512];
        assert!(!is_silence(&samples, 0.01));
    }

    #[test]
    fn is_silence_at_threshold_is_not_silent() {
        // is_silence uses strict < so energy equal to threshold counts as audible
        let samples = vec![0.01f32; 512];
        let rms = rms_energy(&samples);
        assert!(!is_silence(&samples, rms));
    }

    #[test]
    fn is_silence_empty_is_silent() {
        assert!(is_silence(&[], 0.01));
    }
}
