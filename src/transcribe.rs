use anyhow::{Context, Result};
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Transcriber {
    model_path: PathBuf,
    language: Option<String>,
}

impl Transcriber {
    pub fn new(model_path: &std::path::Path, language: Option<String>) -> Result<Self> {
        // Just store the path, don't load the model yet
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
        // Load model
        tracing::info!("Loading whisper model...");
        let load_start = std::time::Instant::now();

        let ctx = WhisperContext::new_with_params(
            self.model_path.to_str().context("Invalid model path")?,
            WhisperContextParameters::default(),
        )
        .context("Failed to load whisper model")?;

        tracing::info!("Model loaded in {:?}", load_start.elapsed());

        // Create state and transcribe
        let mut state = ctx.create_state().context("Failed to create state")?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Speed optimizations
        params.set_n_threads(num_cpus::get() as i32);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        if let Some(ref lang) = &self.language {
            params.set_language(Some(lang));
        }

        let transcribe_start = std::time::Instant::now();
        state
            .full(params, samples)
            .context("Failed to transcribe")?;
        tracing::info!("Transcription took {:?}", transcribe_start.elapsed());

        let num_segments = state.full_n_segments().context("Failed to get segments")?;

        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(&segment);
            }
        }

        // Model is dropped here, freeing memory
        tracing::info!("Model unloaded");

        Ok(text.trim().to_string())
    }
}
