# Flov - Voice to Text

Голосовой ввод текста через локальный Whisper на GPU.

## Использование

1. Положи `ggml-large-v3-turbo.bin` рядом с `flov.exe`
2. Запусти `flov.exe`
3. Зажми **Ctrl+Win** и говори
4. Отпусти — текст вставится в активное поле

## Иконки трея

- Красная — ожидание
- Зелёная — запись
- Жёлтая — транскрипция

## Сборка

```powershell
# Скачать модель
.\download-model.ps1

# Собрать
CMAKE_GENERATOR=Ninja cargo build --release
```

## Требования

- Windows 10/11
- NVIDIA GPU с CUDA
- ~2GB VRAM
