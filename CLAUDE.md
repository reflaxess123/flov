# Flov - Voice to Text Assistant

Windows-приложение для голосового ввода текста. Зажимаешь Ctrl+Win, говоришь, отпускаешь — текст вставляется в активное поле через буфер обмена (Ctrl+V).

## Архитектура

Rust, single binary, без внешних фреймворков. Транскрипция через локальную модель Whisper (whisper.cpp + CUDA).

### Модули (`src/`)

- **main.rs** — точка входа, оркестрация потоков, Windows message loop
- **config.rs** — загрузка `flov.toml` (опционален), дефолты для всех параметров
- **audio.rs** — запись звука через WASAPI (cpal), ресемплинг до 16kHz, FFT-спектр для оверлея
- **transcribe.rs** — загрузка whisper модели и транскрипция аудио в текст
- **hotkey.rs** — глобальный хук клавиатуры (Ctrl+Win), блокирует открытие Start Menu
- **input.rs** — вставка текста через буфер обмена + SendInput (Ctrl+V)
- **postprocess.rs** — пост-обработка текста через OpenRouter API (опционально)
- **overlay.rs** — полупрозрачный оверлей возле курсора с FFT-спектром (eframe/egui)
- **tray.rs** — иконка в трее (цветной кружок), меню с кнопкой "Выход"

### Потоки

1. **Main thread** — Windows message loop, обновление иконки трея
2. **Recording thread** — ожидание хоткея, запись, транскрипция
3. **Text insertion thread** — получает текст по каналу, вставляет через clipboard
4. **Overlay thread** — eframe окно с FFT-визуализацией

### Иконки трея (состояния)

- Красная — ожидание (Idle)
- Зелёная — запись (Recording)
- Жёлтая — транскрипция (Transcribing)

### Пост-обработка текста

Опциональная обработка распознанного текста через OpenRouter API. Включается/выключается через чекбокс в меню трея. Если API ключ не задан в конфиге — пункт меню недоступен (greyed out).

## Сборка

whisper-rs требует CUDA. Из-за несовместимости VS 18 Insiders + CUDA 13.0, нужны env vars:

```powershell
CMAKE_GENERATOR=Ninja
CMAKE_MAKE_PROGRAM="C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja/ninja.exe"
CUDAFLAGS="-allow-unsupported-compiler"
CMAKE_CUDA_FLAGS="-allow-unsupported-compiler"
cargo build --release
```

## Дистрибуция

Для работы нужны 4 файла рядом:

```
flov.exe
ggml-large-v3-turbo.bin    # whisper модель (~1.6 GB)
cublas64_13.dll            # CUDA cuBLAS
cublasLt64_13.dll          # CUDA cuBLAS (зависимость cublas64)
```

Скрипт `download-model.ps1` скачивает модель в `target/release/`.

## Конфиг

`flov.toml` рядом с exe (опционален — без него работает с дефолтами):

```toml
[whisper]
model_path = "ggml-large-v3-turbo.bin"  # относительно exe или абсолютный путь
language = "ru"                          # дефолт: "ru"

[audio]
sample_rate = 16000                      # дефолт: 16000

[openrouter]
api_key = "sk-or-..."                    # API ключ OpenRouter (обязателен для пост-обработки)
model = "openai/gpt-4o-mini"             # дефолт: gpt-4o-mini
system_prompt = "Исправь текст..."       # дефолт: промпт для исправления ошибок распознавания
```

## Зависимости (Cargo.toml)

- **whisper-rs** (0.15, cuda) — транскрипция
- **cpal** — запись аудио (WASAPI)
- **rustfft** — FFT для спектра
- **eframe/egui** — оверлей
- **tray-icon** — иконка в трее
- **windows** — Win32 API (хук клавиатуры, clipboard, SendInput)
- **ureq** — HTTP-клиент для OpenRouter API
- **serde/toml** — конфиг
- **anyhow** — обработка ошибок
- **tracing** — логирование в flov.log
- **num_cpus** — количество потоков для whisper
