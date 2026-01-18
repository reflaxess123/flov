use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::{Arc, Mutex};

pub struct AudioRecorder {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
}

impl AudioRecorder {
    pub fn new(_target_sample_rate: u32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device available")?;

        tracing::info!("Using input device: {}", device.name().unwrap_or_default());

        let config = device
            .default_input_config()
            .context("Failed to get default input config")?;

        tracing::info!(
            "Audio config: {} Hz, {} channels, {:?}",
            config.sample_rate().0,
            config.channels(),
            config.sample_format()
        );

        Ok(Self { device, config })
    }

    pub fn record_while<F>(&self, should_continue: F) -> Result<Vec<f32>>
    where
        F: Fn() -> bool + Send + 'static,
    {
        let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let samples_clone = samples.clone();
        let channels = self.config.channels() as usize;

        let err_fn = |err| {
            tracing::error!("Audio stream error: {}", err);
        };

        let stream = match self.config.sample_format() {
            cpal::SampleFormat::F32 => {
                let config: cpal::StreamConfig = self.config.clone().into();
                self.device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mut samples = samples_clone.lock().unwrap();
                        // Convert to mono by averaging channels
                        for chunk in data.chunks(channels) {
                            let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                            samples.push(mono);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let config: cpal::StreamConfig = self.config.clone().into();
                self.device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mut samples = samples_clone.lock().unwrap();
                        for chunk in data.chunks(channels) {
                            let mono: f32 = chunk.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / channels as f32;
                            samples.push(mono);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let config: cpal::StreamConfig = self.config.clone().into();
                self.device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let mut samples = samples_clone.lock().unwrap();
                        for chunk in data.chunks(channels) {
                            let mono: f32 = chunk.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).sum::<f32>() / channels as f32;
                            samples.push(mono);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => return Err(anyhow::anyhow!("Unsupported sample format")),
        };

        stream.play()?;
        tracing::info!("Audio stream started");

        // Wait while condition is true
        while should_continue() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        drop(stream);
        tracing::info!("Audio stream stopped");

        let samples = Arc::try_unwrap(samples)
            .map_err(|_| anyhow::anyhow!("Failed to unwrap samples"))?
            .into_inner()
            .unwrap();

        // Resample to 16000 Hz if needed (whisper expects 16kHz)
        let native_rate = self.config.sample_rate().0;
        if native_rate != 16000 {
            tracing::info!("Resampling from {} Hz to 16000 Hz", native_rate);
            Ok(resample(&samples, native_rate, 16000))
        } else {
            Ok(samples)
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate().0
    }

    /// Record while condition is true, with frequency spectrum callback (20 bands)
    pub fn record_while_with_spectrum<F, C>(&self, should_continue: F, on_spectrum: C) -> Result<Vec<f32>>
    where
        F: Fn() -> bool + Send + 'static,
        C: Fn(Vec<f32>) + Send + 'static,
    {
        const FFT_SIZE: usize = 2048;
        const NUM_BANDS: usize = 20;

        let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let samples_clone = samples.clone();
        let channels = self.config.channels() as usize;

        // Buffer for FFT analysis
        let fft_buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(FFT_SIZE)));
        let fft_buffer_clone = fft_buffer.clone();

        let err_fn = |err| {
            eprintln!("Audio stream error: {}", err);
        };

        let stream = match self.config.sample_format() {
            cpal::SampleFormat::F32 => {
                let config: cpal::StreamConfig = self.config.clone().into();
                self.device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mut samples = samples_clone.lock().unwrap();
                        let mut fft_buf = fft_buffer_clone.lock().unwrap();
                        for chunk in data.chunks(channels) {
                            let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                            samples.push(mono);
                            fft_buf.push(mono);
                            // Keep only last FFT_SIZE samples
                            if fft_buf.len() > FFT_SIZE {
                                fft_buf.remove(0);
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let config: cpal::StreamConfig = self.config.clone().into();
                self.device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mut samples = samples_clone.lock().unwrap();
                        let mut fft_buf = fft_buffer_clone.lock().unwrap();
                        for chunk in data.chunks(channels) {
                            let mono: f32 = chunk.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / channels as f32;
                            samples.push(mono);
                            fft_buf.push(mono);
                            if fft_buf.len() > FFT_SIZE {
                                fft_buf.remove(0);
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => return Err(anyhow::anyhow!("Unsupported sample format")),
        };

        stream.play()?;

        // FFT setup
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        // Wait while condition is true, computing spectrum
        while should_continue() {
            let buf = fft_buffer.lock().unwrap().clone();
            if buf.len() >= FFT_SIZE {
                // Apply Hann window and compute FFT
                let mut complex_buf: Vec<Complex<f32>> = buf.iter()
                    .enumerate()
                    .map(|(i, &s)| {
                        let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos());
                        Complex::new(s * window, 0.0)
                    })
                    .collect();

                fft.process(&mut complex_buf);

                // Convert to magnitude spectrum (only first half - positive frequencies)
                let magnitudes: Vec<f32> = complex_buf[..FFT_SIZE / 2]
                    .iter()
                    .map(|c| c.norm() / FFT_SIZE as f32)
                    .collect();

                // Group into NUM_BANDS using logarithmic scale
                let mut bands = vec![0.0f32; NUM_BANDS];
                for (i, &mag) in magnitudes.iter().enumerate() {
                    // Logarithmic mapping: lower frequencies get more resolution
                    let freq_ratio = (i as f32 + 1.0) / magnitudes.len() as f32;
                    let band_idx = ((freq_ratio.ln() + 5.0) / 5.0 * NUM_BANDS as f32) as usize;
                    let band_idx = band_idx.min(NUM_BANDS - 1);
                    bands[band_idx] = bands[band_idx].max(mag);
                }

                // Normalize and scale for visualization
                let max_val = bands.iter().cloned().fold(0.0f32, f32::max).max(0.001);
                let spectrum: Vec<f32> = bands.iter()
                    .map(|&b| (b / max_val * 3.0).min(1.0))
                    .collect();

                on_spectrum(spectrum);
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }

        drop(stream);

        let samples = Arc::try_unwrap(samples)
            .map_err(|_| anyhow::anyhow!("Failed to unwrap samples"))?
            .into_inner()
            .unwrap();

        // Resample to 16000 Hz if needed
        let native_rate = self.config.sample_rate().0;
        if native_rate != 16000 {
            Ok(resample(&samples, native_rate, 16000))
        } else {
            Ok(samples)
        }
    }
}

// Simple linear resampling
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut result = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 * ratio;
        let idx = src_idx as usize;
        let frac = src_idx - idx as f64;

        let sample = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac as f32) + samples[idx + 1] * frac as f32
        } else if idx < samples.len() {
            samples[idx]
        } else {
            0.0
        };

        result.push(sample);
    }

    result
}
