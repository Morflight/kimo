#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
: "${KIMO_BUILD_ROOT:?Set KIMO_BUILD_ROOT to an external build/cache directory}"
mkdir -p "$KIMO_BUILD_ROOT"
build_root=$(cd "$KIMO_BUILD_ROOT" && pwd)
image=messense/cargo-xwin@sha256:9856b895265d4966f212228ba64802cf89337e2a2a537aa2533c1b8784cbc81b
common=(--rm --cpus 4 --memory 6g --memory-swap 7g
 --mount "type=bind,src=$repo,dst=/source" --mount "type=bind,src=$build_root,dst=/build"
 -e CARGO_HOME=/build/cargo -e RUSTUP_HOME=/build/rustup
 -e RUSTUP_TOOLCHAIN=1.100.0 -e CARGO_BUILD_JOBS=2
 -e XWIN_CACHE_DIR=/build/xwin -e XWIN_ARCH=x86_64)
docker run "${common[@]}" "$image" rustup toolchain install 1.100.0 --profile minimal
docker run "${common[@]}" -w /source -e CARGO_TARGET_DIR=/build/engine-target  -e CMAKE_TOOLCHAIN_FILE=/build/xwin/cmake/clang-cl/x86_64-pc-windows-msvc-toolchain.cmake  "$image" cargo xwin build --locked --release -p kimo-mcp -p kimo-core --example profile_preview --bin kimo-mcp --target x86_64-pc-windows-msvc
docker run "${common[@]}" -w /source/crates/kimo-gui -e CARGO_TARGET_DIR=/build/gui-target  -e "TAURI_CONFIG=$(cat "$repo/crates/kimo-gui/tauri.fork.json")"  "$image" cargo xwin build --locked --release --target x86_64-pc-windows-msvc --features custom-protocol
