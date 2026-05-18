# Flov на macOS (Apple Silicon)

Гайд по сборке, запуску и архитектурным особенностям маковой версии.
Цель: Apple Silicon only (`aarch64-apple-darwin`). Не fat-binaries, не
Rosetta, не Intel.

## Quick start

```bash
brew install rust cmake node          # если их ещё нет
git clone <repo> && cd flov
cd ui && npm install && cd ..

./dev.sh                              # первый прогон ставит sidecars (~3 мин)
```

При первом запуске macOS попросит **Microphone** и **Accessibility**
permissions:

- **Microphone** — без неё CoreAudio тихо вернёт silent stream
  (NSMicrophoneUsageDescription задано в `src-tauri/Info.plist`).
- **Accessibility** — нужно для `CGEventTap` (хук клавиатуры) и
  `CGEventPost` (синтетический Cmd+V для вставки). Без неё хоткей
  будет молча игнорироваться. Settings → Privacy & Security →
  Accessibility → ✓ flov.

Дефолтный хоткей на маке: **Cmd+Alt** (= Cmd+Option). Переназначить
через Settings → Hotkey.

## Сборка `.app` + `.dmg`

```bash
./scripts/build-bundle.sh
```

Билдит cpu + metal sidecars, стейджит в `src-tauri/binaries/` с
triple-суффиксом `aarch64-apple-darwin`, запускает `tauri build
--bundles app,dmg`. Артефакты:

- `target/release/bundle/macos/flov.app`
- `target/release/bundle/dmg/flov_0.2.0_aarch64.dmg`

Whisper модель (~1.6 GB) **не** внутри бандла — пользователь качает её
из Settings → Models после установки. Модели и конфиг живут в
`~/Library/Application Support/com.flov.app/` (см. §"Хранение
данных").

### Code signing / notarization

Сейчас не настроено — Gatekeeper при первом запуске покажет
"приложение от неизвестного разработчика", пользователю нужно будет
right-click → Open. Для public-релиза понадобятся:

- Developer ID Application сертификат (Apple Developer account)
- `bundle.macOS.signingIdentity` в `tauri.macos.conf.json`
- `xcrun notarytool submit ...` после `tauri build`
- `xcrun stapler staple ...` для embed'а нотаризации в .dmg

### SDK / deployment target — почему build scripts ставят явные env vars

Whisper.cpp's Metal backend использует `MTLResidencySetDescriptor` —
класс который появился в **macOS 15 (Sequoia)** SDK. На машине сборки
с CommandLineTools 16+ SDK = 15.x. Без явного deployment target
получится бинарь который не загружается на Sonoma:

```
dyld: Symbol not found: _OBJC_CLASS_$_MTLResidencySetDescriptor
  Expected in: /System/Library/Frameworks/Metal.framework/...
```

Whisper.cpp **уже** оборачивает все вызовы новых API в
`@available(macOS 15.0, *)` блоки, поэтому код безопасен в рантайме.
Но чтобы линкер сгенерировал **weak references** на эти символы
(вместо обязательных), нужны:

1. `MACOSX_DEPLOYMENT_TARGET=11.0` — Apple's clang добавляет
   `-mmacosx-version-min=11.0` к компилятору C/Obj-C.
2. `CMAKE_OSX_DEPLOYMENT_TARGET=11.0` — отдельно, потому что
   whisper-rs-sys's `build.rs` форвардит env vars начинающиеся с
   `CMAKE_` напрямую в cmake (а `cmake-rs` crate не транслирует
   MACOSX_DEPLOYMENT_TARGET автоматически).
3. `RUSTFLAGS="-L<clang_rt_dir> -lclang_rt.osx"` — Apple's clang при
   `@available()` генерирует runtime вызов `__isPlatformVersionAtLeast`
   который живёт в `libclang_rt.osx.a`. Apple's clang auto-линкует
   эту lib, rustc — нет. Путь подсасывается через
   `clang -print-runtime-dir`.

Всё это автоматически делается в `scripts/build-sidecars.sh` /
`scripts/build-bundle.sh`. Если будешь билдить sidecar напрямую через
`cargo build`, скопируй env vars из тех скриптов.

## Sidecars

Two backends on macOS:

| Backend | Crate | Что делает |
|---|---|---|
| `metal` | `crates/flov-whisper-metal` | Apple Silicon GPU через whisper.cpp Metal backend |
| `cpu` | `crates/flov-whisper-cpu` | CPU fallback (одинаковый бинарь на всех платформах) |

`auto` режим (`flov.toml::backend.choice`) пробует metal → cpu —
первый существующий рядом с бинарём выигрывает.

Vulkan/CUDA sidecars не собираются на маке (build-sidecars.sh их
пропускает). Это намеренно: на Apple Silicon Vulkan = MoltenVK,
который тащит свой Metal-translation слой и медленнее нативного
whisper.cpp Metal. CUDA очевидно неприменим.

## Платформенный код — что отличается от Windows

### `src-tauri/src/hotkey.rs` — macOS hook

`CGEventTap` в выделенном thread'е с собственным `CFRunLoop`. Реагирует
на `KeyDown` + `KeyUp` + `FlagsChanged`. Combo парсится в стиле, аналогичном
Windows (HotkeyDef), но re-mapping в Mac-VK коды и `CGEventFlags`
происходит в `macos_impl::MacCombo::from_combo`.

**Ограничение v1**: callback wrapper из `core-graphics 0.24` не умеет
возвращать NULL (= swallow event). Trigger проходит дальше в систему —
для дефолтного `Cmd+Alt` это безвредно (системного действия нет), но
если пользователь забиндит `Cmd+Space`, Spotlight будет открываться.
Документировать в UI или починить через раздельный raw FFI в v2.

Side-specific хоткеи (например `RCtrl` как одиночный trigger) работают
через FlagsChanged + проверку конкретного `kVK_RightControl`.

### `src-tauri/src/input.rs` — вставка

- Clipboard: `arboard` crate (cross-platform; на Mac использует
  NSPasteboard под капотом).
- Paste hotkey: `CGEvent::new_keyboard_event` с `CGEventFlagCommand`
  для Cmd+V, `CGEvent::post(CGEventTapLocation::HID)`.

Те же permissions что для хоткея.

### `src-tauri/src/ui.rs::position_at_cursor_monitor`

`CGEvent::new(source).location()` даёт глобальную позицию курсора
(top-left origin, points). Перебираем `CGDisplay::active_displays()`,
ищем display чьи bounds содержат точку. Pill положение в logical
units — Tauri конвертирует через `LogicalPosition`. **Нет** scale-
factor умножения как в Windows (где `rcMonitor` в физических пикселях).

### `src-tauri/src/tray.rs`

На macOS: `TrayIconBuilder::icon_as_template(true)`. PNG-глиф рендерится
системой в текущем цвете menu bar (dark/light/transparent) автоматически.
Windows-registry polling thread скомпилен только под `cfg(target_os =
"windows")`.

### `src-tauri/src/paths.rs` — хранение данных

| Файл | Windows | macOS | Linux |
|---|---|---|---|
| flov.toml | `<exe_dir>/flov.toml` | `~/Library/Application Support/com.flov.app/flov.toml` | `$XDG_DATA_HOME/flov/flov.toml` |
| stats.json | `<exe_dir>/stats.json` | то же | то же |
| flov.log | `<exe_dir>/flov.log` | то же | то же |
| models | `<exe_dir>/models/whisper/` | `~/Library/Application Support/com.flov.app/models/whisper/` | то же |

Причина: macOS .app bundle после code-signing **read-only**, любая
запись внутрь ломает подпись. Под Windows installer ставит в
`%LOCALAPPDATA%`, что writable — там оставлено как есть для обратной
совместимости с уже установленными копиями.

## Tauri config layout

Базовый `tauri.conf.json` — кросс-платформенная часть (окна, build
команды, иконки). Платформенные оверрайды Tauri 2 авто-мерджит при
сборке для соответствующего target:

- `tauri.windows.conf.json` — NSIS bundle, externalBin cpu+vulkan,
  webview installer
- `tauri.macos.conf.json` — `.app`/`.dmg` targets, externalBin cpu+metal,
  `app.macOSPrivateApi: true` (нужно для transparent + shadow-disabled pill)
- `src-tauri/Info.plist` — auto-merged tauri-bundler'ом, содержит:
  - `NSMicrophoneUsageDescription`
  - `LSUIElement = true` (agent mode — без Dock-иконки и app-меню)

Cargo features: `tauri = { features = ["macos-private-api", ...] }` —
required парой к `macOSPrivateApi: true` в JSON, иначе tauri-build
откажется компилироваться.

## Debug / диагностика

- Лог: `~/Library/Application Support/com.flov.app/flov.log`
- Tray-иконка: правый верх menu bar (LSUIElement = true, поэтому в
  Dock'е нет).
- Если хоткей не реагирует: `tail -f` лога — `CGEventTapCreate
  failed` означает что нет Accessibility permission.
- `FLOV_BACKEND=cpu ./flov.app/Contents/MacOS/flov` — форсирует
  конкретный sidecar в обход auto.

## Подводные камни TCC + unsigned apps (выученные на боль)

### `AXIsProcessTrustedWithOptions(prompt=true)` сломан для unsigned

Apple's "правильный" API для запроса Accessibility permission. На
signed app — работает: показывает диалог, юзер кликает "Open System
Settings", TCC регистрирует bundle path/id, toggle включается, всё
работает.

На **unsigned** .app bundle: показывает тот же диалог, но TCC
регистрирует **executable** (`Contents/MacOS/flov_app`), а не bundle.
Toggle выглядит включённым, но `AXIsProcessTrusted` возвращает false
бесконечно. Юзер в шоке "у меня же галочка стоит".

**Fix в `hotkey.rs::macos_impl::install_hook`**: вызываем с
`prompt=false`, открываем Settings вручную через
`open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"`,
логируем точный путь до .app, юзер добавляет через `+` руками. Этот
путь работает на любой подписи (или её отсутствии).

### Ad-hoc signing НЕ даёт persistent TCC между rebuilds

Соблазн думать: "подпишу `signingIdentity: -` → TCC сохранит permission
через rebuild'ы, как у нормальных apps". **Не работает**.

TCC матчит запись на:
1. Bundle path (если изменился — отдельная entry)
2. `cdhash` — content directory hash, меняется при любом изменении
   контента (включая код)
3. **Team Identifier** — только для Developer ID signed apps

Ad-hoc signing — `TeamIdentifier=not set`. Значит TCC матч идёт по
`cdhash`, который у каждого rebuild свой. → Каждый rebuild = новая
TCC entry = re-grant required.

Что ad-hoc реально даёт:
- Hardened runtime (security flag, нужен для notarization later)
- Entitlements applied (microphone usage description, audio-input)
- Чуть лучший Gatekeeper UX
- Готовность под notarization

Что **только Developer ID** даёт:
- Стабильный TeamIdentifier → TCC entries persist через rebuilds
- Notarization-eligible (можно убрать "unknown developer" warning
  полностью)
- Распространение через интернет без Gatekeeper трения

### Cpal blocks until Microphone prompt is answered

`cpal::default_input_config()` на macOS висит синхронно, пока юзер
не ответит на mic permission диалог. Если диалог не виден (LSUIElement
agent в фоне), юзер думает что приложение зависло.

В логе между "Using input device" и "Audio config" может быть **1-3
минуты** — это норма. Если хочется лучше UX — temporarily убрать
`LSUIElement` чтобы Dock-иконка была видна на первом запуске.

### TCC keys на полный path .app bundle

Запуск .app из `/Volumes/flov/` (mounted DMG) и из
`/Applications/flov.app` — две **разные** записи в TCC. Юзер дал
permission в DMG → перетащил в Applications → permission потерялся
(другой path → другая entry).

**Onboarding должен**: drag .app **до** первого запуска, никогда не
запускать из mounted DMG.

### Каждый rebuild → re-grant permissions

Это самое больное для dev iteration. Workflow:

```bash
# После build:
killall flov_app
rm -rf /Applications/flov.app
cp -R target/release/bundle/macos/flov.app /Applications/
# Старые TCC entries указывают на старый hash:
tccutil reset Accessibility com.flov.app
tccutil reset Microphone com.flov.app
open /Applications/flov.app
# → System Settings откроется → re-grant → quit/relaunch
```

Замена этому — Apple Developer Program ($99/год), без него потолок.

### Onboarding идеи (не сделано)

Чтобы first-run UX был менее болезненным без Apple Dev — можно:
- Флаг `first_run = true` в `flov.toml`, при нём forced-show
  Settings window с большой кнопкой "Grant Accessibility" + steps
- Polling `AXIsProcessTrusted` каждые 500ms; когда true — emit
  toast "Permission granted, hotkey now active"
- "Help" link на macOS-specific FAQ страницу

Это всё UI работа в Svelte, не блокер для dev/use.

## Что НЕ сделано / открытые вопросы

1. **Suppression** — trigger key не swallow'ится (см. §hotkey).
   Чинить через сырой FFI к `CGEventTapCreate` (минуя
   `core-graphics::CGEventTap` wrapper, который Some/None уравнивает).
2. **Code signing / notarization** — для public-релиза.
3. **CoreML / ANE** — `crates/flov-whisper-metal/Cargo.toml` не
   подключает `coreml` feature. Включить, когда добавим
   `.mlmodelc` файлы рядом с `.bin` (отдельная история, +100 MB к
   модели, но ANE-инференс быстрее).
4. **Accessibility prompt UX** — сейчас при отказе просто молчим в логе.
   Хорошо бы показать окно с кнопкой "Open System Settings → Privacy →
   Accessibility" через `x-apple.systempreferences:` URL scheme.
5. **Universal2 binary / Intel поддержка** — не планируется.
6. **Tray icon tint** на Sequoia+ — возможны нюансы с template image на
   новых системах (например menu bar transparency). Проверять на
   реальной системе.
