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

## API key storage acceptance

Session keys are trimmed and never printed by `Debug`; hints expose only three leading and four trailing characters for keys longer than eight characters. Restoring an absent or empty stored key leaves the session unset. Storage failures preserve a newly supplied session key and report a note; clearing removes the session key even if deleting its persisted copy fails. The OS adapter treats a missing credential as absence and propagates construction and operation errors. Its deterministic tests install the keyring library's mock credential builder only in a dedicated child process, so they never access real credentials. These adapter tests do not certify a live platform keychain service.

Connection tests authenticate only in headers against a localhost fixture, accept missing/non-array model lists as zero visible models, reject unreadable JSON and preserve the HTTP error category. Provider error text is selected from nested error/message, message or error strings, with a 160-character fallback for unstructured bodies. No external provider or user's API key is needed by these tests.

Crash recovery acceptance requires an actual `KIMO_MCP_BIN` binary: the restore/undo test fails when it is missing, and saved project fixtures must contain a tracks array. A missing autosave never produces a recovery offer; an unavailable state destination preserves the existing file. Crash-log tests verify malformed timestamps, unreadable logs, archive failures and both string/non-string panic payloads. Hook payload tests run in a separate process and never submit a report.

Crash-log persistence has a private writer boundary for deterministic storage-failure testing; production still uses `std::fs::write`. Log enumeration uses the directory entry's filename directly and rejects names that cannot be represented as Unicode. The path scrubber tests Windows slash variants and Unix paths explicitly on either host; platform integration remains separately required.

Ledger regression tests cover actor adapters, reset with late findings, bounded Unicode summaries, compact observations, poisoned write-lock recovery and transport/engine/history failures. A late result cannot recreate a reset card; failed undo leaves the revision unchanged, and saving/exporting does not mark the project dirty.

System-tool discovery tests use isolated child processes with synthetic PATH executables and model files. They verify explicit Whisper paths, `whisper-cli` before `whisper-cpp`, the models-bin fallback, documented FFprobe locations outside PATH, and best-model ordering. Platform-path policy is tested for both Windows portable/WinGet/Chocolatey/Scoop locations and Unix locations on either host; these tests do not launch or install the synthetic executables.

Telemetry socket tests verify blank-line handling, invalid JSON status reporting, termination on invalid UTF-8 and a visible error when all public spectator ports are occupied. They use isolated loopback sockets and retain distinct GUI/external source identities.

Portable native-client tests compile a local fixture and verify JSON argv preservation, output trimming and nonzero exit diagnostics. The real-engine supervisor test terminates only its own child, requires a crashed status, then verifies that the next call restarts a new connection and increments the restart count once.

Download unit contracts use a loopback HTTP fixture and temporary directories to verify complete partial-file reuse, HTTP 416 restart, oversized partial replacement, OS/pin filtering and filesystem failures that preserve existing data. They do not download release assets or certify the release manifest.

The transfer loop accepts reader/writer streams so unit tests can assert that read failures remain offline errors and write/flush failures remain I/O errors. Production downloads use the same loop with the HTTP response and partial file.

Shared-engine bridge tests exercise malformed JSON, notifications, default protocol fields, invalid UTF-8 termination, stale discovery files, refusal handshakes and descriptor-write failures on loopback fixtures. Child-process checks isolate environment discovery and relay bytes through real stdin/stdout pipes; they do not attach to a running user application.

The client-handshake transport seam checks write/read/timeout failures without relying on timing races; actual loopback handshakes remain covered. Font discovery is exercised in child processes across override, executable, resource and engine-relative locations. Engine environment tests require missing fonts to be omitted and an existing PATH entry to remain unduplicated.

Viewer error/cache contracts use synthetic tool replies to verify transport errors, malformed summaries, missing output files, fallback hashes/dimensions and draft eviction. These byte fixtures exercise metadata/cache handling; separate real-engine tests remain responsible for rendering and playback evidence.

CLI stream parser contracts assert sparse Claude Code/Codex messages, explicit/default errors, failed tool calls, command outcomes and Unicode-safe truncation. Inputs are synthetic JSON; no installed provider CLI or authenticated account is invoked.

Antigravity parser fixtures also require RUNNING/PENDING tool starts, both cancellation spellings and failed result envelopes to retain failure status, with explicit fallback names and diagnostics for sparse input.

Native command-state regression tests use Tauri’s mock runtime, an isolated settings/recovery directory and an in-memory credential store. They verify agent preference persistence without selection, provider validation, secret redaction, setup cancellation, recovery absence and busy-guard release on unwind. No real provider, credential store or user project is used. These command tests do not establish full desktop coverage.

Viewer range regressions reject malformed numbers, reversed ranges, zero suffixes and empty-file ranges; oversized suffixes clamp to the file and multiple ranges use the first range. Cache tests assert initial emptiness and bounded occupancy after eviction.

The native state suite also starts the required real `KIMO_MCP_BIN` with a synthetic empty project, adds a track, verifies autosave and saved revisions, and discards only its offered snapshot while preserving the newer snapshot. Missing engine binaries fail this test instead of silently skipping it.

Native command handlers accept Tauri’s runtime parameter so the same production handlers can be exercised with its mock runtime. Shipping invocations still infer the normal desktop runtime; the tests use the same state and sidecar logic without opening a user window.

Mock-runtime commands also drive the real sidecar through save/open (including a missing-file error), add-track/undo, new-project and restart. Assertions compare project tracks after undo, persisted project paths and the empty viewer after reset.

Native bridge tests assert JSON-RPC initialization, empty resources, malformed/missing tools and unknown-method errors. Live-event tests distinguish GUI and external sources and preserve agent/turn metadata. A build without an updater key remains off and refuses installation without contacting an update feed.

The shipping IPC registry is shared with mock-webview tests. Required arguments are validated through actual Tauri dispatch before state mutation; successful agent/key settings are then read back with secret redaction. Unknown commands and disabled updates retain their error responses.

CLI process-contract tests compile a small local fixture executable with the test toolchain. They verify prompt delivery, exact argv, streamed session/final replies, authentication/quota errors, registration output and cancellation, without running an installed agent or changing its configuration.

Stopping an active agent kills and reaps only that turn’s child, removes its running slot and reports `Stopped.` rather than a provider exit failure. A failed kill retains the child for normal completion and reports that no stop occurred. The process fixture reproduces the former signal-9 error and asserts the corrected cancellation result.

The stop regression also injects an OS kill refusal, checks that the same child remains alive and owned, and then stops it normally. Only the kill syscall is substituted for this error case.

Discovery/model-cache/sign-in tests run in isolated child processes with fixture CLI overrides and synthetic home/config directories. They test retry after an empty model catalog, caching after success, authentication-status fallbacks and supported configuration locations. Credential fixtures intentionally contain invalid documents: only file existence is checked.
