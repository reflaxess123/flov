use anyhow::{Context, Result};
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Transcriber {
    model_path: PathBuf,
    language: String,
}

impl Transcriber {
    pub fn new(model_path: &std::path::Path, language: String) -> Result<Self> {
        if !model_path.exists() {
            anyhow::bail!("Model file not found: {:?}", model_path);
        }

        tracing::info!("Whisper model path: {:?}", model_path);

        Ok(Self {
            model_path: model_path.to_path_buf(),
            language,
        })
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        tracing::info!("Loading whisper model...");
        let load_start = std::time::Instant::now();

        let ctx = WhisperContext::new_with_params(
            self.model_path.to_str().context("Invalid model path")?,
            WhisperContextParameters::default(),
        )
        .context("Failed to load whisper model")?;

        tracing::info!("Model loaded in {:?}", load_start.elapsed());

        let mut state = ctx.create_state().context("Failed to create state")?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        params.set_n_threads(num_cpus::get() as i32);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        params.set_language(Some(&self.language));

        let transcribe_start = std::time::Instant::now();
        state
            .full(params, samples)
            .context("Failed to transcribe")?;
        tracing::info!("Transcription took {:?}", transcribe_start.elapsed());

        let num_segments = state.full_n_segments();

        let mut text = String::new();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(s) = segment.to_str_lossy() {
                    text.push_str(&s);
                }
            }
        }

        tracing::info!("Model unloaded");

        Ok(text.trim().to_string())
    }
}
