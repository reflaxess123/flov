# Flov

Voice-to-text для Windows. Зажми хоткей, говори, отпусти — текст вставляется
в активное поле через буфер обмена.

Транскрипция локальная (Whisper.cpp на GPU). Опциональная пост-обработка
через OpenRouter (clean-up пунктуации/мата/etc).

## Запуск

1. Скачай или собери `flov.exe` (см. ниже)
2. Положи `ggml-large-v3-turbo.bin` рядом (или скачай через Settings)
3. Запусти `flov.exe` — иконка в трее
4. Зажми **Ctrl+Win**, говори, отпусти

## Settings (правый клик по трею → Open Settings)

- **Models** — каталог Whisper моделей (tiny / base / small / medium / large-v3-turbo)
- **Backend** — выбор GPU sidecar (CUDA / Vulkan / Metal / CPU), Auto = первый доступный
- **Post-process** — OpenRouter API key, модель, системный промпт
- **Hotkey** — любая комбинация (включая одиночный RCtrl)
- **Stats** — heatmap записей по дням

## Сборка из исходников

Требования: Rust toolchain, Node.js, для CUDA — CUDA toolkit и Ninja, для
Vulkan — LunarG SDK.

```powershell
# Дев с hot-reload
.\dev.cmd

# Sidecar бинари (отдельно — workspace excluded)
.\scripts\build-sidecars.ps1                  # все backend'ы
.\scripts\build-sidecars.ps1 -Backend cuda    # один

# Релиз
.\ui\node_modules\.bin\tauri.cmd build
```

Подробности по архитектуре — [CLAUDE.md](CLAUDE.md). По sidecar'ам и
добавлению Mac/Metal — [crates/README.md](crates/README.md).

## Стек

- Tauri 2 + Svelte 5 (frontend, frameless transparent windows)
- Rust + cpal (WASAPI запись) + windows-rs (хук клавиатуры)
- whisper-rs sidecar бинари по backend'у
- OpenRouter HTTP API через ureq
