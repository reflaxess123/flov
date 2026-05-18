#!/usr/bin/env bash
# Release builder for macOS (Apple Silicon). Produces .app + .dmg in
# target/release/bundle/. Mirrors scripts/build-bundle.ps1 in spirit:
#   - builds CPU + Metal sidecars (Vulkan/CUDA are Windows-only)
#   - stages them in src-tauri/binaries/ with the Tauri triple suffix
#   - runs `tauri build --bundles app,dmg`
#
# Usage:
#   ./scripts/build-bundle.sh                  # full build
#   ./scripts/build-bundle.sh --skip-sidecars  # reuse already-built sidecars

set -euo pipefail

skip_sidecars=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-sidecars) skip_sidecars=1; shift ;;
        *) echo "Unknown arg: $1" >&2; exit 1 ;;
    esac
done

root="$(cd "$(dirname "$0")/.." && pwd)"
crates_dir="$root/crates"
target_dir="$root/target"
bin_dir="$root/src-tauri/binaries"

triple="aarch64-apple-darwin"
sidecars=("cpu" "metal")

# Match tauri.macos.conf.json's minimumSystemVersion. See the comment
# in build-sidecars.sh for the dyld symbol issue this prevents.
# build-sidecars.sh sets the same env when it's invoked below, but we
# also need it here because tauri build also recompiles the main app
# (flov_app) with cargo, picking up the same Rust toolchain quirks.
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
export CMAKE_OSX_DEPLOYMENT_TARGET="${CMAKE_OSX_DEPLOYMENT_TARGET:-$MACOSX_DEPLOYMENT_TARGET}"

clang_rt_dir="$(clang -print-runtime-dir 2>/dev/null || true)"
if [[ -d "$clang_rt_dir" && -f "$clang_rt_dir/libclang_rt.osx.a" ]]; then
    export RUSTFLAGS="${RUSTFLAGS:-} -L${clang_rt_dir} -lclang_rt.osx"
fi

build_sidecar() {
    local name="$1"
    local crate="$crates_dir/flov-whisper-$name"
    if [[ ! -d "$crate" ]]; then
        echo "missing crate: $crate" >&2
        exit 1
    fi
    echo ">> building flov-whisper-$name (release)"
    cargo build --release \
        --manifest-path "$crate/Cargo.toml" \
        --target-dir "$target_dir"
}

stage_sidecar() {
    local name="$1"
    local src="$target_dir/release/flov-whisper-$name"
    local dst="$bin_dir/flov-whisper-$name-$triple"
    if [[ ! -f "$src" ]]; then
        echo "expected sidecar not found: $src" >&2
        exit 1
    fi
    cp -f "$src" "$dst"
    chmod +x "$dst"
    echo "   staged $(basename "$dst")"
}

# ── 1. Build sidecars ──────────────────────────────────────────────────
if [[ $skip_sidecars -eq 0 ]]; then
    for s in "${sidecars[@]}"; do
        build_sidecar "$s"
    done
fi

# ── 2. Stage with Tauri's expected naming ─────────────────────────────
mkdir -p "$bin_dir"
# Clean previous Mac-triple staging only (don't disturb Windows staging
# that might coexist in a multi-host clone).
find "$bin_dir" -maxdepth 1 -type f -name "*-$triple" -delete 2>/dev/null || true
for s in "${sidecars[@]}"; do
    stage_sidecar "$s"
done

# ── 3. Build the bundle ────────────────────────────────────────────────
echo ">> tauri build (.app + .dmg)"
tauri="$root/ui/node_modules/.bin/tauri"
if [[ ! -x "$tauri" ]]; then
    echo "Tauri CLI missing at $tauri — run \`npm install\` in ui/ first." >&2
    exit 1
fi

cd "$root"
"$tauri" build --bundles app,dmg

# ── 4. Report bundle locations ─────────────────────────────────────────
dmg_dir="$target_dir/release/bundle/dmg"
app_dir="$target_dir/release/bundle/macos"
if [[ -d "$dmg_dir" ]]; then
    dmg=$(ls -t "$dmg_dir"/*.dmg 2>/dev/null | head -1 || true)
    if [[ -n "$dmg" ]]; then
        sz=$(du -h "$dmg" | cut -f1)
        echo "done."
        echo "DMG: $dmg ($sz)"
    fi
fi
if [[ -d "$app_dir" ]]; then
    app=$(ls -dt "$app_dir"/*.app 2>/dev/null | head -1 || true)
    [[ -n "$app" ]] && echo "App: $app"
fi
