use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TranscribeResponse {
    text: String,
}

pub struct Transcriber {
    url: String,
    client: reqwest::blocking::Client,
}

impl Transcriber {
    pub fn new(url: &str) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .no_proxy()  // Bypass system proxy for localhost
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            url: url.to_string(),
            client,
        })
    }

    pub fn transcribe(&self, samples: &[f32], sample_rate: u32) -> Result<String> {
        // Create temp WAV file
        let temp_file = tempfile::Builder::new()
            .suffix(".wav")
            .tempfile()
            .context("Failed to create temp file")?;

        let path = temp_file.path().to_path_buf();

        // Write WAV
        {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };

            let mut writer = hound::WavWriter::create(&path, spec)
                .context("Failed to create WAV writer")?;

            for &sample in samples {
                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                writer.write_sample(sample_i16)?;
            }

            writer.finalize()?;
        }

        // Send to transcription service
        let file_bytes = std::fs::read(&path)
            .context("Failed to read temp WAV file")?;

        let part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let form = reqwest::blocking::multipart::Form::new()
            .part("file", part)
            .text("language", "ru");

        tracing::info!("Sending audio to transcription service: {}", self.url);

        let response = self.client
            .post(&self.url)
            .multipart(form)
            .send()
            .context("Failed to send request to transcription service")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            anyhow::bail!("Transcription service returned {}: {}", status, body);
        }

        let result: TranscribeResponse = response
            .json()
            .context("Failed to parse transcription response")?;

        tracing::info!("Transcription result: {}", result.text.trim());

        Ok(result.text.trim().to_string())
    }
}
