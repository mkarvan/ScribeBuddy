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
