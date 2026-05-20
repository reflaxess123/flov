# Flov 0.2.2 Windows Reliability Report

Дата: 2026-05-20

## Что исследовали

На Windows были три связанных симптома:

- recording pill/waveform после времени работы или rapid hotkey press мог
  пропадать, хотя запись/транскрипция продолжали работать;
- periodic WebView reload иногда давал видимый blink на долю секунды;
- после установки `0.2.1` Settings из tray `Open Settings` не открывался.

Свежий лог установленной `0.2.1` показал:

```text
INFO creating settings window on demand
ERROR failed to create webview: WebView2 error: WindowsError(Error { code: HRESULT(0x8007139F), message: "The group or resource is not in the correct state to perform the requested operation." })
INFO settings window created and focused
```

Важно: Tauri мог вернуть window wrapper даже после того, как внутренний
WebView2 controller не создался. Поэтому UI не появлялся, а следующие клики
в tray только фокусировали уже битую обёртку.

## Root cause

Для pill main window в `tauri.conf.json` задан кастомный
`additionalBrowserArgs`, чтобы отключить Chromium native occlusion и
background throttling:

```text
--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion
--disable-backgrounding-occluded-windows
--disable-renderer-backgrounding
--disable-background-timer-throttling
```

Settings создавался лениво без этих args, но в той же WebView2 user data
folder. Wry/Tauri документирует Windows-ограничение: WebView2 instances с
разными environment settings (`additionalBrowserArgs`,
`browserExtensionsEnabled`, `scrollBarStyle`) должны использовать разные
data directories. Иначе второй WebView может падать с `0x8007139F`.

Это совпало с публичными WebView2 issue: ошибка `0x8007139F` встречается
при создании второго WebView, несовместимых environment parameters, DPI или
compatibility state.

## Исправления

- Версия поднята до `0.2.2`, потому что `0.2.1` уже был установлен и
  воспроизвёл Settings regression.
- Settings по-прежнему не объявлен в `tauri.conf.json`; он создаётся
  лениво через `ui::open_settings_window`.
- На Windows Settings получает отдельную WebView2 data directory:
  `user_data_dir()/webview-settings`.
- Если `get_webview_window("settings")` возвращает существующую window
  wrapper, но `window.eval("void 0")` падает, wrapper закрывается и
  Settings создаётся заново.
- `CLAUDE.md`, `README.md`, `AGENTS.md` обновлены с этим WebView2
  ограничением.

## Pill / waveform fixes from 0.2.1

- Main pill больше не скрывается через `window.hide()` на Windows.
  В idle HWND остаётся transparent/click-through с alpha `0`, а DOM пустой.
- Backend хранит snapshot текущего pill state и отдаёт его через
  `pill_frontend_ready`, чтобы reload не мог потерять `state-changed`.
- Frontend регистрирует listeners до `pill_frontend_ready`, сбрасывает stale
  hide/error timers и защищает delayed callbacks sequence counter'ом.
- Periodic reload перенесён в отдельный worker и пропускается во время
  active recording/transcribe cycle.
- Blink закрыт двухслойно:
  - `WEBVIEW2_DEFAULT_BACKGROUND_COLOR=00000000` до создания WebView2;
  - `window.show()` происходит при Windows alpha `0`, а alpha `255`
    возвращается только после frontend `tick()` + `requestAnimationFrame()`.

## Проверки

Перед сборкой `0.2.2` пройдены:

- `cargo check`
- `cargo test`
- `npm run check`
- `npm run build`

После финальной документации нужно ещё раз прогнать `cargo clippy`,
`git diff --check`, собрать installer и только после этого делать GitHub
release через `gh release create`.

## Остаточные риски

- WebView2 `0x8007139F` может также возникать из-за Windows compatibility
  mode, повреждённого WebView2 runtime/profile или DPI mismatch. Эти случаи
  не лечатся кодом приложения полностью, но текущий найденный конфликт
  `additionalBrowserArgs`/data dir устранён.
- Если Settings всё ещё не откроется после `0.2.2`, первым делом смотреть
  новые строки `settings webview data dir:` и `settings window exists but
  webview is not usable` в `flov.log`.
