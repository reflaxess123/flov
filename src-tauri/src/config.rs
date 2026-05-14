use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub whisper: WhisperConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub openrouter: OpenRouterConfig,
}

#[derive(Debug, Deserialize)]
pub struct WhisperConfig {
    #[serde(default = "default_model_path")]
    pub model_path: PathBuf,
    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model_path: default_model_path(),
            language: default_language(),
        }
    }
}

fn default_language() -> String {
    "ru".to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_openrouter_model")]
    pub model: String,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_reply_system_prompt")]
    pub reply_system_prompt: String,
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_openrouter_model(),
            system_prompt: default_system_prompt(),
            reply_system_prompt: default_reply_system_prompt(),
        }
    }
}

fn default_openrouter_model() -> String {
    "openai/gpt-4o-mini".to_string()
}

fn default_system_prompt() -> String {
    "Ты — редактор голосовых транскрипций. Ты получаешь сырой текст, распознанный из речи через Whisper. Твоя задача — превратить его в чистый, грамотный текст, готовый к отправке в чат или другую языковую модель.\n\nПравила обработки:\n\n1. Исправь ошибки распознавания речи, восстанови правильные слова по контексту.\n\n2. Замени всю обсценную и ненормативную лексику на нейтральные эквиваленты, точно передающие эмоцию и смысл. Не смягчай и не теряй интенсивность высказывания — только убери мат.\n\n3. Сохрани все английские слова и термины как есть. Автор — программист и математик, в его речи регулярно встречаются англоязычные термины: названия технологий, функций, библиотек, математических концепций. Не транслитерируй и не переводи их.\n\n4. Расставь знаки препинания: запятые, точки, тире, двоеточия. Следуй правилам русской пунктуации.\n\n5. Раздели текст на логические абзацы по смыслу. Каждая отдельная мысль или тезис — новый абзац.\n\n6. Сохрани точный смысл и цель высказывания. Автор формулирует задачи, вопросы и инструкции для других языковых моделей. Критически важно не потерять, не исказить и не додумать его намерение. Не добавляй ничего от себя.\n\n7. Выведи только готовый текст. Без комментариев, пояснений, приветствий и маркдаун-разметки.".to_string()
}

fn default_reply_system_prompt() -> String {
    "Ты — персональный ассистент для составления ответов на сообщения. Ты получаешь два входа:\n\n1. КОНТЕКСТ — текст из буфера обмена (сообщения от собеседника, переписка, письмо).\n2. ИНСТРУКЦИЯ — голосовая команда автора, описывающая, что и как ответить.\n\nПравила:\n\n1. Напиши ответ строго от лица автора. Ответ должен звучать естественно, как будто его написал живой человек в мессенджере.\n\n2. Следуй инструкции автора по содержанию и тону. Если автор сказал «отмажься», «согласись», «вежливо откажи» — делай именно это.\n\n3. Стиль: дружелюбный, лаконичный, без канцелярита и формальностей, если инструкция не требует иного. Не пиши как робот.\n\n4. Не используй мат, даже если он есть в инструкции или в исходных сообщениях.\n\n5. Не добавляй ничего, о чём автор не просил. Не додумывай факты, обещания, детали.\n\n6. Выведи только текст ответа, готовый к отправке. Без комментариев, вариантов, пояснений.".to_string()
}

fn default_model_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    exe_dir.join("ggml-large-v3-turbo.bin")
}

fn default_sample_rate() -> u32 {
    16000
}

impl Config {
    pub fn load() -> Result<Self> {
        let exe_dir = std::env::current_exe()
            .context("Failed to get executable path")?
            .parent()
            .context("Failed to get executable directory")?
            .to_path_buf();

        let config_path = exe_dir.join("flov.toml");

        if !config_path.exists() {
            return Ok(Config {
                whisper: WhisperConfig::default(),
                audio: AudioConfig::default(),
                openrouter: OpenRouterConfig::default(),
            });
        }

        let config_str = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {:?}", config_path))?;

        let mut config: Config = toml::from_str(&config_str)
            .context("Failed to parse config")?;

        // Resolve relative model_path against exe directory
        if config.whisper.model_path.is_relative() {
            config.whisper.model_path = exe_dir.join(&config.whisper.model_path);
        }

        Ok(config)
    }
}
