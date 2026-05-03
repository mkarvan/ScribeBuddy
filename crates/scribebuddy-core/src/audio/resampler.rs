use anyhow::Result;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

pub struct AudioResampler {
    resampler: SincFixedIn<f32>,
    input_rate: f64,
    output_rate: f64,
}

impl AudioResampler {
    pub fn new(input_rate: u32, output_rate: u32, chunk_size: usize) -> Result<Self> {
        let input_rate = input_rate as f64;
        let output_rate = output_rate as f64;

        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        let resampler = SincFixedIn::new(
            output_rate / input_rate,
            2.0,
            params,
            chunk_size,
            1,
        )?;

        Ok(Self {
            resampler,
            input_rate,
            output_rate,
        })
    }

    pub fn resample(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        let frames = vec![input.to_vec()];
        let output = self.resampler.process(&frames, None)?;
        Ok(output.into_iter().next().unwrap_or_default())
    }

    pub fn input_rate(&self) -> f64 {
        self.input_rate
    }

    pub fn output_rate(&self) -> f64 {
        self.output_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_44100_to_16000() {
        let chunk = (44100.0_f32 * 3.0) as usize;
        let mut r = AudioResampler::new(44100, 16000, chunk).unwrap();
        let out = r.resample(&vec![0.0f32; chunk]).unwrap();
        let expected = (16000.0_f32 * 3.0) as usize;
        assert!(
            (out.len() as i64 - expected as i64).unsigned_abs() < (expected / 100) as u64,
            "got {} expected ~{}",
            out.len(),
            expected
        );
    }

    #[test]
    fn resample_48000_to_16000() {
        let chunk = (48000.0_f32 * 3.0) as usize;
        let mut r = AudioResampler::new(48000, 16000, chunk).unwrap();
        let out = r.resample(&vec![0.0f32; chunk]).unwrap();
        let expected = (16000.0_f32 * 3.0) as usize;
        assert!(
            (out.len() as i64 - expected as i64).unsigned_abs() < (expected / 100) as u64,
            "got {} expected ~{}",
            out.len(),
            expected
        );
    }
}
