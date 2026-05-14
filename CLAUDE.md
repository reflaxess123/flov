# Flov - Voice to Text Assistant

Windows push-to-talk: зажимаешь Ctrl+Win, говоришь, отпускаешь — текст
вставляется в активное поле через буфер обмена (Ctrl+V).

UI: Tauri 2 webview с Svelte 5. Frontend рендерит Mac-style капсулу-pill у
курсора с морф-анимацией (FFT столбики при записи → синусные волны при
транскрипции → polished pill после AI пост-обработки). Backend: Rust,
Win32 хук клавиатуры + cpal для записи + spawn sidecar для транскрипции.

## Layout

```
flov/
├── Cargo.toml          # workspace root: members = ["src-tauri"], exclude = ["crates/*"]
├── .cargo/config.toml  # CMake/CUDA env для whisper.cpp build (нужно для CUDA sidecar)
├── flov.toml           # дев-конфиг (gitignored), копия в target/debug/
├── src-tauri/          # main app (flov_app.exe)
│   ├── Cargo.toml
│   ├── tauri.conf.json # window config: 800x200, decorations off, transparent, click-through
│   ├── icons/          # tray.png (32x32, чёрный глиф; rt.invert на dark theme)
│   ├── capabilities/   # Tauri 2 permissions
│   └── src/
│       ├── main.rs        # entry, вызывает flov_lib::run
│       ├── lib.rs         # оркестрация: загрузка config, инициализация Recorder/Transcriber/HotkeyHook,
│       │                  # spawn recording_loop, Tauri Builder
│       ├── audio.rs       # WASAPI запись через cpal, ресемплинг 16kHz, FFT-спектр для оверлея
│       ├── hotkey.rs      # глобальный хук клавиатуры Ctrl+Win, блокирует Start Menu
│       ├── input.rs       # вставка через clipboard + SendInput Ctrl+V
│       ├── transcribe.rs  # spawn sidecar (Command::new), pipe f32 PCM в stdin, читает stdout
│       ├── postprocess.rs # OpenRouter API (опционально, через тогл в трее)
│       ├── config.rs      # flov.toml parser, write_backend_choice через toml_edit
│       ├── tray.rs        # Tauri 2 native tray, Backend меню (radio), theme-aware иконка
│       └── ui.rs          # Tauri commands + Win32 helpers (position_at_cursor_monitor, force_click_through)
├── crates/             # sidecar transcription backends (см. crates/README.md)
│   ├── README.md       # архитектура sidecars + инструкция для нового backend
│   ├── flov-whisper-cuda/    # NVIDIA, whisper-rs feature cuda
│   ├── flov-whisper-vulkan/  # AMD/Intel iGPU, whisper-rs feature vulkan
│   ├── flov-whisper-cpu/     # fallback
│   └── flov-whisper-metal/   # Apple Silicon (ещё нет, инструкция в crates/README.md)
├── ui/                 # SvelteKit (adapter-static, port 1420)
│   └── src/
│       ├── routes/+page.svelte  # Tauri event listener: state-changed, audio-spectrum, polished-text
│       └── lib/
│           ├── Pill.svelte         # morph transition (circle pop → capsule expand)
│           ├── Waveform.svelte     # 20 статичных FFT столбиков, currentColor
│           ├── SineWave.svelte     # 3 переплетающихся синусных волны при транскрипции
│           └── PolishedPill.svelte # breathe анимация для финальной капсулы
└── scripts/
    └── build-sidecars.ps1  # билдит все crates/flov-whisper-*, копирует в target/{debug,release}/
                             # + cublas DLLs для CUDA
```

## Транскрипция через sidecars

Главное архитектурное решение: транскрипция вынесена в **отдельные бинари
по backend'у**. `flov_app.exe` не знает про whisper-rs / CUDA / Vulkan и не
требует CUDA toolchain для своей сборки. См. `crates/README.md` для полной
картины — wire protocol, build, добавление нового sidecar.

Селекция backend'а в `transcribe::resolve_sidecar`:
1. `FLOV_BACKEND` env var (debug override)
2. `[backend].choice` из flov.toml (тогглится из tray-меню)
3. `auto` → priority `[cuda, vulkan, metal, cpu]`, первый существующий рядом с exe

При смене backend'а из tray не нужен рестарт — Transcriber резолвит sidecar
на каждый transcribe call через shared `Arc<Mutex<String>>`.

## UI flow

Threads:
1. **Main** — Win message loop, tray events
2. **Recording loop** (`lib.rs::recording_loop`) — ждёт хоткей, эмитит state events
   (`state-changed: idle|recording|transcribing|polished`), пишет аудио в Recorder,
   вызывает Transcriber, шлёт текст
3. **WebView** — Svelte рендерит Pill реагирующий на state events

Tauri events:
- `state-changed: "idle"|"recording"|"transcribing"|"polished"` → Pill переключает контент
- `audio-spectrum: number[]` (20 bands) → Waveform столбики
- `polished-text: string` → PolishedPill показывает финальный текст

Tauri commands (frontend → backend):
- `polished_shown` — анимация polished pill закончилась, можно paste'ить
- `hide_window` — после morph-out transition прячем окно (избегаем мигание)

Tray меню:
- **Backend**: Auto / CUDA / Vulkan / Metal / CPU (radio, greyed для отсутствующих sidecars)
- **Post-process via OpenRouter** (greyed без API key)
- **Quit**

## Сборка

Main app:
```powershell
cd src-tauri && tauri dev      # дев с hot reload
tauri build                    # release
```

Sidecars (отдельная команда — workspace excluded):
```powershell
.\scripts\build-sidecars.ps1                  # все backend'ы (release)
.\scripts\build-sidecars.ps1 -Backend cuda    # один
.\scripts\build-sidecars.ps1 -Profile debug   # debug build
```

Скрипт сам стейджит exe + cublas DLLs (для CUDA) в `target/debug` и
`target/release`, чтобы `tauri dev` и packaged build их подхватили.

CUDA build env (уже в `.cargo/config.toml`, не надо ручками):
- `CMAKE_GENERATOR=Ninja`, `CUDAFLAGS=-allow-unsupported-compiler`,
  `CXXFLAGS=/Zc:preprocessor`, `CMAKE_CUDA_FLAGS="-Xcompiler /Zc:preprocessor"`

Vulkan build требует LunarG SDK: `winget install KhronosGroup.VulkanSDK`,
после установки `VULKAN_SDK` подхватывается из system env (новый shell).

## Дистрибуция

Минимум для CUDA-варианта:
```
flov.exe
flov-whisper-cuda.exe
cublas64_13.dll
cublasLt64_13.dll
ggml-large-v3-turbo.bin     # модель ~1.6 GB
icons/tray.png
flov.toml                    # опционален
```

Можно класть несколько sidecar бинарей рядом — pick'ается на runtime.
Vulkan/CPU sidecars не требуют дополнительных DLL.

## flov.toml

```toml
[whisper]
model_path = "ggml-large-v3-turbo.bin"  # относительно exe или абсолютный
language = "ru"

[audio]
sample_rate = 16000

[backend]
choice = "auto"  # "auto" | "cuda" | "vulkan" | "metal" | "cpu"
                 # перетирается из tray-меню через toml_edit (комменты сохраняются)

[openrouter]
api_key = "sk-or-..."        # без ключа пост-обработка недоступна
model = "openai/gpt-4o-mini"
system_prompt = "..."
reply_system_prompt = "..."
```

## Зависимости (src-tauri/Cargo.toml)

- **tauri** (2, features = ["tray-icon", "image-png"])
- **cpal** — WASAPI запись
- **rustfft** — FFT для спектра
- **windows** (0.61) — Win32 API (хук, clipboard, MonitorFromPoint, Registry)
- **ureq** — HTTP клиент для OpenRouter
- **toml** + **toml_edit** — конфиг (read + surgical write)
- **image** — recolor tray PNG для dark theme
- **anyhow**, **tracing**, **tracing-subscriber**, **serde/serde_json**

Sidecar crates (`crates/flov-whisper-*/Cargo.toml`):
- **whisper-rs** (0.16, разные features per backend)
- **anyhow**, **num_cpus**
