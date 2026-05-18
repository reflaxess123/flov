#!/usr/bin/env bash
# One-shot dev runner for macOS / Linux. Equivalent of dev.cmd.
#
# Tauri build script validates that every `externalBin` entry in
# tauri.{platform}.conf.json exists with the expected triple suffix,
# *even during `cargo check`*. So we stage real sidecars before
# starting tauri dev — same pattern the Windows scripts assume.

set -euo pipefail

root="$(cd "$(dirname "$0")" && pwd)"
cd "$root"

# Build + stage sidecars on first run; subsequent runs reuse the
# cached binaries (cargo's own incremental compilation handles
# rebuilds when sources change).
need_stage=0
case "$(uname -s)" in
    Darwin)
        triple="aarch64-apple-darwin"
        for s in cpu metal; do
            if [[ ! -f "src-tauri/binaries/flov-whisper-$s-$triple" ]]; then
                need_stage=1; break
            fi
        done
        ;;
    Linux)
        # Linux dev path is not wired up yet; pop a hint and continue.
        echo "Note: Linux sidecar staging not implemented in this script."
        ;;
esac

if [[ "$need_stage" -eq 1 ]]; then
    echo ">> staging sidecars (one-time, ~3 min cold)"
    "$root/scripts/build-sidecars.sh"
    if [[ "$(uname -s)" == "Darwin" ]]; then
        mkdir -p src-tauri/binaries
        for s in cpu metal; do
            cp -f "target/release/flov-whisper-$s" \
                  "src-tauri/binaries/flov-whisper-$s-aarch64-apple-darwin"
        done
    fi
fi

exec ./ui/node_modules/.bin/tauri dev
