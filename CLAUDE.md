# Flov - Voice to Text Assistant

Windows push-to-talk: зажимаешь хоткей (по умолчанию Ctrl+Win), говоришь,
отпускаешь — текст вставляется в активное поле через буфер обмена (Ctrl+V).

UI: Tauri 2 webview с Svelte 5. Frontend рендерит Mac-style капсулу-pill у
курсора с морф-анимацией. Backend: Rust, Win32 хук клавиатуры + cpal для
записи + spawn sidecar для транскрипции.

## Layout

```
flov/
├── Cargo.toml          # workspace root: members = ["src-tauri"], exclude = ["crates/*"]
├── .cargo/config.toml  # CMake/CUDA env для whisper.cpp build (нужно для CUDA sidecar)
├── flov.toml           # дев-конфиг (gitignored), копия в target/debug/
├── dev.cmd             # one-click рестарт: PATH+=cargo, tauri dev из root
├── docs/DESIGN.md      # дизайн-нотки по pill / settings
├── src-tauri/          # main app (flov_app.exe)
│   ├── Cargo.toml
│   ├── tauri.conf.json # 2 окна: main (pill, frameless transparent) + settings (1240×880, frameless)
│   ├── icons/          # tray.png (32x32, чёрный глиф; tray.rs.invert на dark theme)
│   ├── capabilities/   # Tauri 2 permissions (settings.json — для settings window)
│   └── src/
│       ├── main.rs        # entry, вызывает flov_lib::run
│       ├── lib.rs         # оркестрация: загрузка config, инициализация Recorder/Transcriber/HotkeyHook,
│       │                  # spawn recording_loop, Tauri Builder
│       ├── audio.rs       # WASAPI запись через cpal, ресемплинг 16kHz, FFT-спектр для оверлея
│       ├── hotkey.rs      # глобальный WH_KEYBOARD_LL хук, парсер combo строк (Ctrl+Win, RCtrl и т.д.),
│       │                  # live re-bind через static Mutex<Option<HotkeyDef>>
│       ├── input.rs       # вставка через clipboard + SendInput Ctrl+V
│       ├── transcribe.rs  # spawn sidecar (Command::new), pipe f32 PCM в stdin, читает stdout
│       ├── postprocess.rs # OpenRouter API (опционально, тогл из settings); логирует HTTP status/body
│       ├── config.rs      # flov.toml parser, surgical write через toml_edit (сохраняет коментарии)
│       ├── tray.rs        # Tauri 2 native tray (Open Settings / Quit), theme-aware иконка, multi-state
│       ├── ui.rs          # Tauri commands + Win32 helpers (position_at_cursor_monitor,
│       │                  # force_click_through, disable_native_window_rounding для settings)
│       ├── models.rs      # каталог Whisper моделей (tiny..large-v3-turbo)
│       ├── models_cmd.rs  # Tauri commands для скачивания / выбора модели
│       ├── state_cmd.rs   # Tauri commands для backend / postprocess / hotkey / stats settings
│       └── stats.rs       # JSON-лог записей по дням (для heatmap)
├── crates/             # sidecar transcription backends (см. crates/README.md)
│   ├── README.md       # архитектура sidecars + полный гайд для Mac/Metal sidecar
│   ├── flov-whisper-cuda/    # NVIDIA, whisper-rs feature cuda
│   ├── flov-whisper-vulkan/  # AMD/Intel iGPU, whisper-rs feature vulkan
│   ├── flov-whisper-cpu/     # fallback
│   └── flov-whisper-metal/   # Apple Silicon (ещё не создан, гайд в crates/README.md)
├── ui/                 # SvelteKit (adapter-static, port 1420)
│   └── src/
│       ├── routes/
│       │   ├── +page.svelte         # main pill window — слушает state-changed/audio-spectrum events
│       │   └── settings/+page.svelte # Settings window — drag-strip + 2-col grid (left: Models+Backend+Stats,
│       │                              # right: Postprocess+Hotkey)
│       └── lib/
│           ├── Pill.svelte         # capsule с morph transition (вжух scale + width expand),
│           │                       # cross-fade currentColor recording↔transcribing,
│           │                       # hasRevealedRef latch — линия рисуется один раз на mount
│           ├── AudioWave.svelte    # SVG-стек из MAX_LINES=3 path'ей, sin-window pinned at endpoints,
│           │                       # per-line desync (phase/freq/speed), opacity-controlled visibility,
│           │                       # stroke-dashoffset для left-to-right reveal
│           └── settings/           # Models / Backend / Postprocess / Stats компоненты
└── scripts/
    └── build-sidecars.ps1  # билдит все crates/flov-whisper-*, копирует в target/{debug,release}/
                             # + cublas DLLs для CUDA
```

## Транскрипция через sidecars

Главное архитектурное решение: транскрипция вынесена в **отдельные бинари
по backend'у**. `flov_app.exe` не знает про whisper-rs / CUDA / Vulkan и не
требует CUDA toolchain для своей сборки. См. `crates/README.md` для полной
картины — wire protocol, build, добавление нового sidecar (Mac/Metal гайд
там же).

Селекция backend'а в `transcribe::resolve_sidecar`:
1. `FLOV_BACKEND` env var (debug override)
2. `[backend].choice` из flov.toml (тогглится из Settings → Backend)
3. `auto` → priority `[cuda, vulkan, metal, cpu]`, первый существующий рядом с exe

При смене backend'а из Settings не нужен рестарт — Transcriber резолвит
sidecar на каждый transcribe call через shared `Arc<Mutex<String>>`.

## UI flow

Threads:
1. **Main** — Win message loop, tray events, Tauri command handlers
2. **Recording loop** (`lib.rs::recording_loop`) — ждёт хоткей, эмитит state events
   (`state-changed: idle|recording|transcribing`), пишет аудио в Recorder,
   вызывает Transcriber, опционально OpenRouter, шлёт текст через `input::type_text`
3. **WebView pill** — Svelte рендерит Pill реагирующий на state events
4. **WebView settings** — независимое окно, ходит в backend через Tauri commands

Tauri events (backend → pill webview):
- `state-changed: "idle"|"recording"|"transcribing"` → Pill переключает контент
- `audio-spectrum: number[]` (20 bands) → AudioWave амплитуда

Tauri commands (frontend → backend):
- `hide_window` — после morph-out transition прячем pill окно
- `list_models` / `download_model` / `delete_model` / `set_active_model` — модели
- `get_backend_state` / `set_backend_choice` — backend
- `get_postprocess_config` / `set_postprocess_config` / `set_postprocess_enabled`
- `get_hotkey` / `set_hotkey` — хоткей с live re-bind
- `get_stats`

Pill (`Pill.svelte` + `AudioWave.svelte`):
- Recording: 3 переплетающихся wavy линий, амплитуда от FFT mic spectrum
- Transcribing: схлопывается до одной линии в accent цвете (lime на dark, чёрный на light),
  амплитуда sequenced (приглушение → recolor → ramp up)
- Появление: morph (backOut scale pop "вжух" → width expand с overlap),
  линия рисуется через stroke-dashoffset слева направо. Ровно один раз на mount —
  переход recording→transcribing НЕ запускает re-stroke
- Уход: транзишн зеркалится через css(t) reverse

Tray меню (минимальное):
- **Open Settings**
- **Quit**

(модели, backend, post-process, hotkey — всё в Settings window)

## Hotkey

Парсер в `hotkey.rs` поддерживает:
- Generic модификаторы: `Ctrl`, `Alt`, `Shift`, `Win` (любая сторона)
- L/R-specific: `LCtrl`, `RCtrl`, `LAlt`, `RAlt`, `LShift`, `RShift`, `LWin`, `RWin`
- Триггер-keys: те же модификаторы (как single-key) + `A-Z`, `0-9`, `Space`, `Enter`,
  `Tab`, `Esc`, `Delete`, `Backspace`

Combo формат: `Ctrl+Win`, `RCtrl`, `Ctrl+Shift+K` и т.д. Последний токен — trigger
(KEYDOWN запускает запись, KEYUP останавливает). Все предыдущие — modifier'ы,
которые должны быть нажаты во время trigger'а.

UI capture (`Postprocess.svelte`): keydown буферит pending combo, keyup коммитит.
Это позволяет одинаково записывать и одиночные клавиши (RCtrl), и комбо.

## Сборка

Main app (из root):
```powershell
.\dev.cmd                       # дев с hot reload (PATH+=cargo, tauri dev)
.\ui\node_modules\.bin\tauri.cmd build  # release
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

### NSIS installer (`scripts/build-bundle.ps1`)

Скрипт строит single-file `flov_<version>_x64-setup.exe`:
- `.\scripts\build-bundle.ps1` — CPU + Vulkan (zero deps на машине user'a)
- `.\scripts\build-bundle.ps1 -IncludeCuda` — добавляет CUDA sidecar
  и cublas DLLs (требует CUDA toolkit при сборке, NVIDIA driver у юзера)

Tauri требует sidecar бинари с triple-суффиксом
(`flov-whisper-cpu-x86_64-pc-windows-msvc.exe`) в
`src-tauri/binaries/`. Скрипт это стейджит автоматически из
`target/release/`. Cublas DLLs идут в `binaries/runtime/` и
конфигурируются через `bundle.resources` в `tauri.conf.json`.

Whisper модель (~1.6 GB) НЕ в installer — пользователь скачает её
через Settings → Models после установки.

### Manual layout (без installer)

Минимум для CUDA-варианта:
```
flov_app.exe
flov-whisper-cuda.exe
cublas64_13.dll
cublasLt64_13.dll
ggml-large-v3-turbo.bin     # или скачивается через Settings
icons/tray.png
flov.toml                    # опционален — Settings создаст
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

[hotkey]
combo = "Ctrl+Win"  # см. hotkey.rs парсер для синтаксиса

[openrouter]
api_key = "sk-or-..."        # без ключа пост-обработка недоступна
model = "openai/gpt-4o-mini"
system_prompt = "..."
```

Все поля редактируются surgical через toml_edit — комментарии сохраняются.

## Зависимости (src-tauri/Cargo.toml)

- **tauri** (2, features = ["tray-icon", "image-png"])
- **cpal** — WASAPI запись
- **rustfft** — FFT для спектра
- **windows** (0.61) — Win32 API (хук, clipboard, MonitorFromPoint,
  DwmSetWindowAttribute для отключения native rounding на Settings, Registry)
- **ureq** — HTTP клиент для OpenRouter
- **toml** + **toml_edit** — конфиг (read + surgical write)
- **image** — recolor tray PNG для dark theme
- **anyhow**, **tracing**, **tracing-subscriber**, **serde/serde_json**

Sidecar crates (`crates/flov-whisper-*/Cargo.toml`):
- **whisper-rs** (0.16, разные features per backend)
- **anyhow**, **num_cpus**
