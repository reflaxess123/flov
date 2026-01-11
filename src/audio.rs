use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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
