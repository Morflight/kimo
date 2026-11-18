# Maintaining Morflight Kimo

Source: https://github.com/Morflight/kimo (default development branch: `main`).
Upstream: https://github.com/naman-sharma12345/kimo.
Baseline: v9.0.0, `8ca3a54d8b99d31c871d270f029379787cce7b54`.
Original MIT license, copyright and third-party notices remain in place.
This fork is independently maintained; upstream README/product claims are upstream documentation.

## Development

```sh
git clone --branch main https://github.com/Morflight/kimo.git
cd kimo
git remote add upstream https://github.com/naman-sharma12345/kimo.git
git switch main
bash tools/fork/test-preview.sh
```

This is a solo-maintainer repository: commit and push directly to `main`; feature branches and pull requests are optional. Run the relevant tests locally and inspect CI on every push. Fetch upstream with `git fetch upstream --tags`, then integrate a reviewed upstream revision and verify it with the same checks. Do not reset main onto upstream. Keep the full history and notices. The first two fork commits preserve the prior index/worktree split: delivery fixes, then the cumulative measured playback/preparation work and tests. The original temporary checkout was left untouched.

Keep media, real editing projects/history, bridge credentials, caches and binaries outside this repository. Tests must use synthetic fixtures or explicitly supplied local paths. Two decoder capture artifacts inherited from upstream were removed from the maintained tree; upstream history is retained unchanged.

## Tests

Requires Node 22 and Bash, no npm install:

```sh
bash tools/fork/test-preview.sh
# Equivalent isolated environment:
docker run --rm --mount "type=bind,src=$PWD,dst=/src,readonly" -w /src node:22 \
  bash -c 'for n in playback_delivery playback_sync playback_buffer playback_rebuffer playback_prepare prepare_ui; do node crates/kimo-gui/tests/${n}_test.mjs || exit; done'
```

`fork-preview` CI runs the same regression script on pushes/PRs to main. It covers delivery failures/timeouts, sync, startup reserve, underrun without rewind, preparation cancellation/reuse and editor preparation controls. These controller tests do not verify physical audio or desktop presentation.

For wider native validation (Rust, FFmpeg/ffprobe and platform build dependencies required):

```sh
cargo test --locked --profile gate -p kimo-core --lib
cargo test --locked --profile gate -p kimo-core --test preview_stream
cargo test --locked --manifest-path crates/kimo-gui/Cargo.toml --release --lib
```

Inspect upstream `tests/gate_fast.sh`, `tests/gate_full.sh` and known-failure lists before interpreting broader failures. The full core suite has not been certified for this baseline.

## Windows builds from WSL

The tested toolchain was Rust 1.100 with the pinned cargo-xwin image in `tools/fork/build-windows.sh`. Both workspace and GUI dependency lockfiles are committed. GUI dependency license auditing remains pending; committing its resolved lockfile is not a license audit or release approval.

Before allocating caches/build outputs on the editing workstation, run `video-storage check --reserve-gib 20` and verify the intended drive is online and has headroom. Choose a build directory outside the source repository:

```sh
export KIMO_BUILD_ROOT=/path/on/build-drive/kimo-build
bash tools/fork/build-windows.sh
```

The script installs Rust 1.100.0 in that build directory, builds the engine plus `profile_preview`, and builds the custom-protocol GUI. Docker, network access and sufficient build space are prerequisites. Outputs are under `$KIMO_BUILD_ROOT/engine-target/x86_64-pc-windows-msvc/release` and `gui-target/x86_64-pc-windows-msvc/release`. This portable script adapts the successful local build recipes; it has syntax validation but was not used for a new complete build during fork setup.

`tauri.fork.json` supplies the separate `dev.morflight.kimo` identity and disables upstream updater keys/endpoints. Use this override for every fork GUI build, including native builds via `TAURI_CONFIG`. Do not ship upstream updater configuration. Engine discovery/download defaults are still inherited: use an explicit isolated engine/profile launcher as in the existing local test installation, and verify engine selection before testing. A standalone distributable with fork-owned engine downloads/signing/update infrastructure is future work. Do not publish upstream `v*` tags or run inherited release workflows as a fork release procedure.

Linux/native engine build: `cargo build --locked --release -p kimo-mcp`. GUI native build additionally needs Tauri platform dependencies documented in `crates/kimo-gui/README.md`; that historical README's uncommitted-lockfile note does not apply to this fork.

## Baseline behavior and evidence

The fork retains the 32-frame startup reserve; underruns do not rewind. Windows RGBA transport uses a bounded 1 MiB pipe. One decode batch can overlap rendering, safe ordinary previews can use stride two, and plain opaque clips can bypass float composition with conservative fallback. Preview cadence targets about 30 fps and is never configured below 27 fps unless the timeline itself is lower.

Edit mode can prepare up to 32 frames after the first image and one idle second. Cancellation is visible and sticky until explicitly restarted. Edits, scrub, navigation and hiding cancel it. Preparation uses one render worker, single-thread codecs/filters, no prefetch, 64 MiB decode cache (not total process memory), background Windows priorities, 20 ms frame pauses and a five-second engine watchdog. Audio waits for Play. Automatic work is limited to plain HD projects, source pixel counts up to 4K and at most 256 clips.

Prior local Windows release builds and native checks passed. The 30-second proxy-backed fixture prepared in 3.32 s, waited 0.152 s after Play, then ran at 28.6–30.6 fps without subsequent rebuffer. Native preparation peaked around 179 MiB engine RSS plus 43 MiB decoder RSS. The 0.48 ms cancel API response is not a process-termination timing claim. Pixel comparisons covered direct rendering, sparse decoding, prefetch and cuts.

Those measurements are inherited evidence from the tested local session, not newly measured during fork setup. The harness used real BMP/PCM with a simulated audio clock; physical audio, desktop canvas presentation, layered projects and unprepared originals are not certified. Private reports, fixtures, projects and binaries remain outside the public source fork.

Fork setup verification (2026-11-18): all six JavaScript regression suites passed under Node 22, both production JS files passed syntax checks, build scripts passed Bash syntax checks, and all 17 original changed/new source files matched the reviewed checkout byte for byte. No new Rust build or full native suite was run during setup.

The subsequent portability commit changes only the buffer test default source path to resolve relative to the test file; the original exact source snapshot remains in the preceding commits. GitHub CI exposed the former Docker-only `/src` assumption.
