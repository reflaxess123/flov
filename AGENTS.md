# AGENTS.md

## Hard rules

- Никогда не используй Playwright, Puppeteer, Chrome MCP, headless
  Chrome/Edge, screenshots, network capture или браузерный debug без явной
  команды пользователя.
- Фронт проверяй сначала через код, route config, imports, API calls, built
  assets и public files.
- Не откатывай пользовательские изменения без прямого запроса.

## Project context

Flov — Tauri 2 + Svelte 5 desktop app для push-to-talk voice-to-text.
Backend Rust: global hotkey, cpal recorder, sidecar Whisper backends,
OpenRouter postprocess, tray/settings commands.

Windows artifacts:

- installer: `target/release/bundle/nsis/flov_<version>_x64-setup.exe`;
- installed app/log: `%LOCALAPPDATA%\flov\flov_app.exe`,
  `%LOCALAPPDATA%\flov\flov.log`;
- current hotkey default: `Ctrl+Win`, user config may override to `RCtrl`.

macOS artifacts:

- `.app` / `.dmg` from `scripts/build-bundle.sh`;
- app data/log: `~/Library/Application Support/com.flov.app/`;
- default hotkey: `Cmd+Alt`.

## Windows WebView2 gotchas

Do not reintroduce eager hidden Settings window creation. Settings must be
created lazily through `ui::open_settings_window`.

Do not create Settings in the same WebView2 data directory as the main pill
when environment settings differ. The main pill uses custom
`additionalBrowserArgs` to disable Chromium occlusion/background throttling;
Settings uses a dedicated `webview-settings` data dir. Without this,
WebView2 can fail the second controller with:

```text
HRESULT(0x8007139F): The group or resource is not in the correct state
```

Tauri can leave a window wrapper behind even when the internal WebView2
controller failed. Keep the `window.eval("void 0")` validation before
reusing an existing Settings window.

## Pill lifecycle gotchas

On Windows, do not hide the main pill HWND with `window.hide()` in idle.
The window stays OS-visible, transparent, click-through, and alpha `0`.
Svelte controls visual presence with DOM mount/unmount.

The frontend must register Tauri event listeners before calling
`pill_frontend_ready`. That command returns the backend state snapshot so a
reload cannot lose a recording/transcribing/error state.

Periodic WebView reload must stay gated by `ui::overlay_active()`,
`RECORDING_CYCLE_ACTIVE`, hotkey mode, and quiet time. A reload during active
record/transcribe can recreate the disappearing waveform bug.

## Verification baseline

Before shipping Windows changes:

```powershell
cargo check
cargo test
cargo clippy --all-targets --all-features
npm run check --prefix ui
npm run build --prefix ui
git diff --check
.\scripts\build-bundle.ps1 -SkipSidecars
```

Do not publish a GitHub release until the installer has been rebuilt from a
clean tree and the tag/version matches `src-tauri/Cargo.toml`,
`src-tauri/tauri.conf.json`, and `Cargo.lock`.
