# Flov - Voice to Text Assistant

Голосовой ввод текста с транскрипцией через Whisper и опциональной обработкой через GLM API.

## Возможности

- **Hotkey Ctrl+Win** - зажми для записи, отпусти для транскрипции
- **Whisper** - локальная транскрипция на GPU (CUDA)
- **GLM API** - опциональное улучшение текста (грамматика, пунктуация, замена мата)
- **Анимация** - волновая анимация во время записи
- **Tray** - иконка в трее с чекбоксом для включения GLM обработки

## Установка

### Windows

#### 1. Установи зависимости

```powershell
# Rust
winget install Rustlang.Rustup

# LLVM (для компиляции whisper.cpp)
winget install LLVM.LLVM

# CMake
winget install Kitware.CMake

# CUDA Toolkit (для GPU ускорения)
# Скачай с https://developer.nvidia.com/cuda-downloads
```

#### 2. Перезапусти терминал чтобы PATH обновился

#### 3. Склонируй и собери

```powershell
git clone https://github.com/reflaxess123/flov.git
cd flov
cargo build --release
```

#### 4. Скачай модель Whisper

```powershell
mkdir target\release\models
# Скачай модель с https://huggingface.co/ggerganov/whisper.cpp/tree/main
# Рекомендую: ggml-large-v3-turbo.bin (~1.5GB) для лучшего качества
# Или: ggml-small.bin (~500MB) для баланса скорости/качества
```

#### 5. Создай конфиг

```powershell
copy flov.toml.example target\release\flov.toml
# Отредактируй flov.toml - укажи свой API ключ если нужна GLM обработка
```

#### 6. Запусти

```powershell
.\target\release\flov.exe
```

### Linux

> **Note:** Пока не тестировалось на Linux. Теоретически должно работать с изменениями.

#### 1. Установи зависимости

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install build-essential cmake llvm-dev libclang-dev
sudo apt install libasound2-dev  # для cpal (аудио)

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# CUDA Toolkit
# Скачай с https://developer.nvidia.com/cuda-downloads
```

#### 2. Собери

```bash
git clone https://github.com/reflaxess123/flov.git
cd flov
cargo build --release
```

#### 3. Скачай модель и создай конфиг

```bash
mkdir -p target/release/models
# Скачай модель whisper в target/release/models/

cp flov.toml.example target/release/flov.toml
# Отредактируй конфиг
```

**Проблемы на Linux:**
- Hotkey через Windows API - нужно переписать на X11/Wayland
- Tray icon - может потребовать libappindicator
- Overlay - нужно переписать на GTK/X11

## Конфигурация

```toml
[whisper]
model_path = "models/ggml-large-v3-turbo.bin"
language = "ru"  # или "" для автоопределения

[audio]
sample_rate = 16000

[api]
# Опционально - для GLM обработки текста
endpoint = "https://api.z.ai/api/coding/paas/v4/chat/completions"
key = "YOUR_API_KEY"
model = "glm-4.5-air"
```

## Использование

1. Запусти `flov.exe`
2. В трее появится красная иконка
3. Зажми **Ctrl+Win** и говори
4. Отпусти - текст вставится в активное поле
5. Правый клик по иконке в трее:
   - **GLM обработка** - включить/выключить улучшение текста
   - **Выход** - закрыть приложение

## Модели Whisper

| Модель | Размер | Качество | Скорость |
|--------|--------|----------|----------|
| ggml-tiny.bin | ~75MB | Низкое | Быстро |
| ggml-small.bin | ~500MB | Среднее | Средне |
| ggml-medium.bin | ~1.5GB | Хорошее | Медленно |
| ggml-large-v3-turbo.bin | ~1.5GB | Отличное | Средне |

Скачать: https://huggingface.co/ggerganov/whisper.cpp/tree/main

## Требования

- Windows 10/11 (Linux экспериментально)
- NVIDIA GPU с CUDA (для ускорения)
- ~2GB VRAM для large модели
- Микрофон

## License

MIT
