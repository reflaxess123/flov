$ModelUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin"
$OutDir = "$PSScriptRoot\target\release"
$OutPath = "$OutDir\ggml-large-v3-turbo.bin"

if (!(Test-Path $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
}

if (Test-Path $OutPath) {
    Write-Host "Model already exists: $OutPath"
    exit 0
}

Write-Host "Downloading ggml-large-v3-turbo.bin (~1.6 GB)..."
$ProgressPreference = 'SilentlyContinue'
Invoke-WebRequest -Uri $ModelUrl -OutFile $OutPath
$ProgressPreference = 'Continue'

Write-Host "Done: $OutPath"
