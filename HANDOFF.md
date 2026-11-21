# Handoff: branch `testing-strategy` (PR Morflight/kimo#2)

Owned by the coordinating agent session. Update it with each change of state; delete it in the commit that makes PR #2 ready to merge (it is a working document, not product documentation).

State on 2026-11-20, branch head `0c4acbd` (pushed). Read AGENTS.md and docs/TESTING_STRATEGY.md first: they are normative.

## 1. Where things are

| What | Where |
|---|---|
| This branch, checked out | WSL worktree `~/kimo-build/wt-testing-strategy` (Windows: `\\wsl.localhost\Ubuntu-22.04\home\osboxes\kimo-build\wt-testing-strategy`) |
| Main folder `C:\Users\adrie\Documents\Codex\2026-11-18\kimo-maintained-fork\outputs\kimo` | Checked out on `split-core`, used by ANOTHER session (core crate split, ADR 0002). Do not commit, switch branches or edit files there. |
| Coverage / test runner | `~/kimo-build/tools/covrun2.sh` (one Docker job at a time via flock, 2 CPUs, 4 GB + 2 GB swap, `CARGO_BUILD_JOBS=1`, opt-level 1). Build this worktree with `KIMO_REPO=/home/osboxes/kimo-build/wt-testing-strategy`. Targets `kimo-core:all`, `kimo-core:lib`, `kimo-mcp:lib`, `kimo-mcp:all`. |
| Other tools | `~/kimo-build/tools/{timings.sh,bench2.sh,fastrun.sh,regions.py,group-*.txt}`; CI logs downloaded to `~/kimo-build/ci/` |
| CI | `.github/workflows/quality.yml`: Rust jobs run on Windows only (`8a8b98b`); fast suite on push, heavy suite on the PR. Branch protection on `main` requires the 8 remaining checks, both Windows rust-coverage jobs included. |

Commits on this branch since `main`: one-binary integration tests (`b622f37`), opt-level 1 for workspace crates (`e9ab27b`), Windows-only Rust CI (`8a8b98b`), Windows fixes (`80c67b8`, `eba8aab`, `0c4acbd`), plus the earlier testing-strategy and WIP coverage commits.

Not yet checked: the CI result of `0c4acbd` (expected: the EXR and external_refs failures gone).

## 2. Machine rules (the owner's PC)

- Never run cargo/docker directly; always `covrun2.sh` (or the bench/timings scripts, same lock and caps). One heavy job at a time.
- Run WSL scripts as files: `wsl bash -c "tr -d '\r' < /mnt/c/<script>.sh | bash"`. PowerShell mangles inline `$`, quotes and `|`.
- `wsl.exe` can time out with `Wsl/Service/0x8007274c` under build load: fragmented VM memory leaves no contiguous 512 KB block for the Hyper-V socket channel (`page allocation failure: order:7 ... vmbus_alloc_ring` in dmesg). Mitigated on 2026-11-20: `vm.min_free_kbytes=262144`, `vm.compaction_proactiveness=50`, live and in `/etc/sysctl.d/99-wsl-vmbus.conf`, and `kernelCommandLine` in `%USERPROFILE%\.wslconfig` (applies at the next WSL restart). Do not restart WSL without the owner. If a call times out, wait 20-40 s and retry.
- Ask before pushing, before downloading anything, and before changing system or repository settings.

## 3. What needs fixing

### 3.1 Windows fast suite (every push): 1 open, 1 flaky

**`find_by_loudness::min_and_max_lufs_together_bound_a_range`** (crates/kimo-core/tests/all/find_by_loudness.rs)
- Fails on Windows only (FFmpeg 9.0.2 gyan.dev build), on every run since before this branch:
  `import_media: cannot probe C:\...\Temp\/kimo-fbl-<pid>-a.wav: ...: End of file`.
- The file is a 1 s, 48 kHz, 16-bit stereo 1 kHz sine at -5 dBFS written by `tone_wav()` in the test (a hand-built 44-byte RIFF header). The same test's -25 and -45 dBFS files, and the other tests' -10/-30 dBFS files, probe fine. So it depends on content: likely a heuristic in FFmpeg 9's WAV demuxer (codec/format detection inside the data chunk), not a path or locking problem.
- The probe is `render::probe()`: `ffprobe -v error -select_streams v:0 -show_entries stream=width,height,r_frame_rate,duration:format=duration -of json <path>`.
- Mechanism (2026-11-21, FFmpeg 7.1 in the Linux image, `-v debug` on the same bytes): after `Format wav probed ... score=99`, the WAV demuxer probes the PCM payload as a codec and reports `detected mpegts with score=100` for the -5 dBFS file only (-10/-25 dBFS: `adp` score 25). The 1 kHz period at 48 kHz stereo 16-bit is exactly 192 bytes (the M2TS packet size) and near-peak samples have the high byte 0x47 (the TS sync byte). FFmpeg 7.1 ignores the detection; FFmpeg 9 presumably acts on it, hence `End of file`. Not verified on FFmpeg 9 yet.
- Next step: reproduce with a static FFmpeg 9 Linux build (BtbN GitHub releases, ~80 MB, verify the checksum; the owner said "not now" on 2026-11-20: ask again). Try `ffprobe` on the generated file with `-v debug`, and compare with `-f wav` forced. Fix in the product if real WAVs can hit it (e.g. force the demuxer for `.wav`), not by changing the tone level.

**`delivery::collect_and_relink`**: flaky on Windows (failed once in the fast run of `80c67b8`, passed in the heavy run of the same commit). Frame 0 of the relinked export differs: that frame holds text drawn with a system font copied into the project. Suspect a font copy or read racing Windows Defender during collect. Watch it; if it recurs, add a retry or a verification to the collect copy (product code).

### 3.2 Windows heavy suite (PRs; required to merge)

Results of the PR run on `80c67b8`: 3086/3096 pass. Besides find_by_loudness, EXR and external_refs (fixed in `0c4acbd`):

| Test | Message | Lead |
|---|---|---|
| `director_small::text_over_subject_fixes_each_caption_page_on_its_own` | `only page two sits over the bright subject, page one should not be flagged` (left 0) | The test names `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`, which does not exist on Windows: text falls back to another font and the geometry changes. Use a font the repo ships (`fonts/`, KIMO_FONTS) or a per-OS system font. Test-side fix. **Fixed 2026-11-21 (pending CI)**: the test pins `tests/fonts/Montserrat-Var.ttf` (passes on Linux, heavy profile). The default font of `add_captions`/`add_caption_cues` (`editorial_tools.rs:134`) is that Linux path: on Windows every caption falls back (product defect, not fixed). |
| `director_small::lint_edit_scans_a_long_clip_without_rescanning_from_its_start_each_sample` | `lint_edit took 24.7s on a 60s single clip (ceiling 18.0s for this build)` | The ceiling was set on Linux. Profile the per-sample decode window on Windows (process spawn cost per sample?). Never raise the ceiling to pass (docs/TESTING_STRATEGY.md §7); fix the cost or get the owner's decision. |
| `quality_v8120::preset_byte_identity_report` | `clean must stay byte-identical` | The log shows `font 'Montserrat'/'Anton'/'Inter' was not found` although CI runs `bash fonts/fetch.sh`: check that the preset fonts are found on Windows (KIMO_FONTS / font lookup path). Also timed out (600 s) on Linux before the Windows switch. **Cause found 2026-11-21 (fix pending CI)**: the Windows job moves `CARGO_TARGET_DIR` off the checkout, so `text::font_dirs`'s executable-relative lookup never reaches `<repo>/fonts`; `quality.yml` now sets `KIMO_FONTS` to the fetched fonts, as release.yml does. Whether byte identity then holds is not verified. |
| `track_mask_accuracy::noise_strict_iou_gate` | `strict noise gate not met` | Also fails on Linux. The printed IoUs are ~0.9998; read the full gate output in the log to see which bound fails. |
| `preview_stream::bench_1080p_three_tracks_plays_at_25_fps_and_scrubs_under_150_ms` | `cancelled: call exceeded its time limit` (decode window 1623 ms) | Performance on the hosted runner; also fails on Linux. |
| `focus_series::prereg_r_real_clip` | `set KIMO_FOCUS_REAL to the real clip ... NotPresent` | Needs a real clip CI does not have. Per AGENTS.md a missing dependency is not a pass: needs an owner decision (provide the clip to CI, or a policy change). |
| `render_bench::render_bench` | timeout (600 s) | Also times out on Linux. |

Desktop heavy (crates/kimo-gui): `tests::updater_checks_owned_feeds_and_rejects_unsigned_download_before_installation` panics at `src/main.rs:1251` with `Os { code: 10035, kind: WouldBlock }`. On Windows an accepted socket inherits the listener's non-blocking mode; the test's local server probably sets the listener non-blocking and then reads the accepted stream as if blocking. Fix: `stream.set_nonblocking(false)` after `accept()`, in the test or the code it exercises.

### 3.3 Coverage gate: 100% lines/functions/regions per file (fast suite)

From the Windows fast run of `80c67b8` (`tools/quality/check_rust_coverage.py`):
- kimo-core: 393 files below 100%; 4,628 lines, 1,911 functions and 29,896 regions uncovered in total.
- Largest gaps (uncovered lines / total): render.rs 154/1763, masks.rs 135/431, auto_proxy.rs 129/455, ext/auto_edit.rs 108/574, shards.rs 104/271, ext/editorial_tools.rs 98/1263, color/scope_image.rs 92/775, ext/audio_samples.rs 92/445, effects.rs 83/300, particle_flow.rs 80/920, vfx_graph.rs 78/363, ext/director_tools.rs 77/1241, preview_stream.rs 75/561, glass.rs 71/71, ext/audio_filters.rs 68/246.
- The report stops at the first failing crate: kimo-cli and kimo-mcp were not reported in that run; check them too.
- kimo-gui (desktop): main.rs 1628/1777 lines, 296/344 functions, 4090/4411 regions; cli_agents.rs 978/982 lines on some runs. Plan: move logic out of the Tauri command bodies and closures into plain functions with unit tests; put the native dialogs (`pick`, `reveal`) behind a small trait; one successful mock IPC call per command (tauri::test, like the existing test near main.rs:1358). Building needs the image `kimo-quality-desktop:rust1.100` under the same lock and caps.
- Policy (owner): unit tests first in `#[cfg(test)] mod tests` of each src file; one or two functional tests per feature on tiny media; no coverage-chasing integration tests; unreachable code is simplified, not tested. Source groups for parallel work: `~/kimo-build/tools/group-{effects,render3d,infra,tracking,editorial}.txt` (at most 3 agents at once, disjoint files).
- `split-core` will move these files into new crates: coordinate with that session before large coverage work on files it is moving.

### 3.4 Product defects found, not fixed

- Default output paths hard-coded to `/tmp` (wrong on Windows): `crates/kimo-core/src/ext/matte_tools.rs:22` (`/tmp/kimo_matte_...`), `ext/refine_matte.rs:38` (`/tmp/kimo_refine...`), `ext/undistort.rs:25` (`/tmp/kimo_undi...`). Use `std::env::temp_dir()`; update the feature contract and add a test.
- Test-only defaults with `/tmp` in `particle_flow.rs` (`KIMO_PV3/PV4_FRAMES`, heavy dumps) are harmless but Windows-unfriendly.

### 3.5 Before merging PR #2

1. Every required check green, heavy suite included (§3.1-3.3).
2. Delete this file.
3. The `split-core` branch (other session) is based on `e42fd2a` and needs `8a8b98b..HEAD` of this branch (Windows-only CI and the Windows fixes); `quality.yml` and `FEATURES.md` will need a merge.
