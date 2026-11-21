# Native test environment

The complete native suite executes on Linux and Windows, including explicitly ignored proofs: the `fast` nextest profile (every push) runs every test outside the heavy register, the `heavy` profile (pull requests and pushes to `main`) runs everything (see `docs/TESTING_STRATEGY.md`). Missing inputs, failed subprocesses and unmeasured files remain failures. Native tests mutate process-wide environment; nextest isolates each test in its own process so they run in parallel without changing their assertions or thresholds.

Editorial audio, caption and synchronization fixtures use the operating system's temporary directory instead of a hard-coded `/tmp` prefix. The same ffmpeg generators, media contents and assertions execute on both platforms. Fixture filenames retain a process identifier and case name. This changes only test path portability, not production media behavior.

Other inherited native prerequisites and failures remain under investigation, including color benchmark input files, visualization output directories and shell-dependent child-process fixtures. This document is not evidence that the complete native suite passes.

The audio allocation test follows the long-edit policy introduced in `69086a4`: the PCM cap is `max(64 MiB, memory_budget / 4)`. The owner explicitly confirmed retaining that policy. Tests now assert exact boundary values, invalid/overflow sample counts, and a timeline one second beyond the host cap returning the specific resource-limit error before PCM allocation. The obsolete 64 MiB maximum assertions are replaced; production limits, coverage thresholds and other acceptance thresholds are unchanged.

Coverage CI supplies the previously manual color benchmark input by converting the bundled astronaut photograph to one 1920×1080 RGBA frame and checking its exact byte length. Color benchmark inputs and diagnostic dumps use the OS temporary directory on both platforms. The comparisons between reference and current algorithms retain their existing exactness/tolerance assertions; no golden output is replaced. The CI also supplies separate temporary output directories for CSM, PBR and primitive visualization proofs. Producing these frames does not by itself establish visual quality.

The loudness reference test reads one JSON object from FFmpeg stderr, which can contain trailing progress/summary diagnostics. It now also requires FFmpeg itself to exit successfully. The strict `< 0.15 LU` comparison is unchanged; the previous failure was JSON trailing text, not a measured loudness discrepancy.

The mux command-line tests create their staging transactions in the OS temporary directory with case/process-specific target names. They must also pass with a read-only source checkout. Their exact AAC bitrate, shortest-stream and output argument assertions are unchanged; transaction cleanup still runs through the real implementation.

The ordinary SDR export tags the filtered video frames as well as the encoder options. FFmpeg 7.1.5 otherwise drops transfer/primaries metadata even when `-color_trc` and `-color_primaries` are supplied. The `setparams` stage follows the existing scale conversion and declares the same configured transfer, BT.709 primaries/matrix and limited range; it does not perform a second pixel conversion. The actual MP4/ffprobe acceptance still requires all four exact tags. Command-line tests now include that required metadata stage and continue to assert the default/preset encoder options.

The early-encoder-exit fixture waits for its own child's observed exit before testing the subsequent write. A fixed 30 ms sleep could race process startup under load. The wait remains bounded by the existing two-second encoder deadline, and the assertions still require both the original stderr diagnostic and the early-exit classification. Encoder timeout/cleanup tolerances are unchanged.

MCP live-preview cadence tests explicitly use `observe: "numbers"` for the timed mutation burst/stream, matching GUI-originated edits. Default `fast` observation synchronously renders its own before/after images and can stretch a nominal 50 ms cadence beyond a second. The tests assert the returned observation mode and retain the original burst-frame bound, three-second stream, minimum updates and coalescing checks. Other live-preview tests retain default observation and independently decode the real PNGs.

Telemetry byte-comparison/load proofs run with observation off so per-call observation timing and temporary image paths cannot invalidate an otherwise byte-identical transport response; observation and real live images remain covered by their own suites. Explicit frame forwarding is matched by call ID, keeping automatic previews distinct. Export checks parse the export reply and assert its actual bytes, duration and output path alongside the telemetry counts. Default-handshake tests fail when required loopback ports are occupied and include unexpected stderr in failure diagnostics.

Default-port transport equality uses observation off to exclude volatile timings and temporary image paths, and an explicitly located system font for its text fixture; stderr must still be empty. The headless demo cadence case uses numeric observations so a burst is not stretched by synchronous before/after image rendering. Export progress requires FFmpeg and fails if it is absent.

The one-to-one state-lane load tests disable observation image generation explicitly; all 600/8,000 calls, backpressure stalls, request identities, terminal/error events and project-change counts remain required. Observation image behavior is covered separately.

Tool-description regressions enforce the existing first-sentence limit across the entire registry. Longer capability details remain after the summary; caption composition, visual inspection, motion review, reference comparison, review findings and detector fallback documentation are checked explicitly.

On Windows, LLVM export may exceed the operating system's command-line length limit when the full workspace has hundreds of test executables. `tools/quality/rust_coverage_report.py` retries only that never-executed export using a temporary response file. It retains every original object and flag, writes the same JSON artifact and propagates the export exit status. No coverage source, threshold or test is removed.

Golden text manifests check out with LF endings on every platform. This keeps byte-for-byte recipe hash comparisons stable on Windows without regenerating expected hashes or normalizing away differences in the test.

The overlap-pipeline tests encode their own six-second moving-pattern H.264 fixture instead of referencing a developer's private absolute path. They retain the exact exported-frame count, per-frame hash equality and cache-isolation assertions. Peak-memory checks read Linux VmHWM or the Windows parent process's PeakWorkingSet64, retaining the 1,800,000 KiB limit. CI provides output paths for all camera, particle, transition, UV and foundation rendering proofs.

The original WebM regression uses OpenCV Extra's public `faceocc2.webm` from commit `ebf9366b3845efe1e54bf2440cf238fd68905bab` (4,134,756 bytes). CI verifies SHA-256 `5ffedeca4fb3acfd65e6f6aa71f016f748628727be1e24914fde2198b42089c5` before setting `KIMO_FACEOCC2_WEBM`; no synthetic substitute is used for that original-file regression.

Caption fixtures compile a small native recognizer executable rather than invoking Unix shell scripts. The JSON token timings, language selection, confidence values and recorded arguments remain the same, with no real speech model or provider required.

Instrumented CI tests use test-profile optimization level 2, retaining debug assertions, all targets, ignored proofs and the existing performance thresholds. The three-hour job limit is unchanged. This avoids measuring an unoptimized software renderer while preserving the tested behavior and coverage inventory.

Process-lifecycle unit tests use a compiled native child for stdout/stderr, blocked pipes, delayed exit and stdin capture. They keep their existing 100 ms deadlines, two-second cleanup ceilings, exact output and bounded-stderr checks. Reaping is checked through `/proc` on Linux and the Windows process table on Windows, rather than assuming `/proc` exists everywhere.

Export stderr readers, preview warm-up timers and vision fixtures start through the cancellation-token helper. The existing worker-cancellation and raw-thread checks remain unchanged.

Preview background decode policy is carried by its render cache and copied explicitly by snapshots. Linear blend scratch uses the existing three-frame bounded pool, keyed by both width and height, instead of an additional thread-local cache. Regression tests verify independent cache policies, same-area/different-shape buffer reuse, dirty-pixel clearing and identical blends after prior renders; the thread-local inventory limits remain unchanged.

Natural-language tool search treats shorter/shorten/longer/length as duration intent and ignores common connective words. Existing top-hit and filler-word ranking requirements are retained.

The registry golden records eight reviewed contract changes: six shortened tool-summary first sentences with their details retained; the existing load_project restore argument (5cbf5f7); and preview_stream warming (77b1c91). Only those eight registry hashes change. Frame and project golden hashes stay unchanged and remain exact comparisons.

A failed encoder keeps its actual stderr when the export frame channel closes, allowing disk-full/codec classification and chunk-preserving recovery. Render/decode errors keep precedence when they caused the stop. The error-code golden updates only the discovered effect count (185 to 189); all codes, parameters and other hints remain identical.

The external-reference audit now classifies the caption/chart call specifications, pinned vision model files, SVG/CSV inputs and fast-export reads. Project-derived media still passes through Project::resolve; caller inputs are expanded into native project data. Every source file remains scanned and exact open-site counts remain enforced, including test fixture reads.

The director initial profile keeps its 36-tool/29,900-byte limits: review remains listed, while detailed look/watch/compare_reference inspection remains discoverable through search_tools and kimo_describe and callable in the full registry. Motion-blur sampling uses resolved transforms; crowding fixes sample local animation at the reported time; motion-path key detection reads key metadata while positions continue through resolve_xform. No transform-lint file exclusions or waiver lines are added.

The IPC harness uses Tauri's platform-local origin (http://tauri.localhost on Windows/Android, tauri://localhost elsewhere). This preserves real command authorization and argument validation on Windows rather than disabling the ACL.

Whisper fallback discovery takes an explicit directory provider so both platform policies can be checked against owned fixture folders on Windows and Linux, including the missing-file case. The production provider remains the existing system-directory search.

The crowded-text fixture positions the second line from its measured ink top, then asserts the actual five-pixel gap before checking the unchanged crowding rule. The picture-scan regression checks both a distinct clip midpoint and a midpoint shared with the regular grid: seven distinct samples in each case, exactly one decode preparation per sample, and a complete reported scan.

Viewer engine tests require KIMO_MCP_BIN to point to an existing test binary and fail explicitly when it is unavailable. A render interrupted while holding the render mutex is followed by frame and draft requests, checking that poison recovery produces the expected cache bytes.

The Studio setup acceptance test waits for the asynchronously refreshed Ready indicator after the setup dialog closes; its existing timeout and text assertions remain unchanged.

Delivery font fixtures use the engine's platform-aware system font discovery and still exercise actual TrueType embedding restrictions and collected-file hashes. The export-memory regression reads physical RAM from Windows CIM or Linux MemTotal, retaining the 85% hard ceiling and exact worker-count assertions. Variable-frame-rate fixture encoding uses FFmpeg's output-specific fps_mode=vfr; decoded-frame equality remains exact.

Viewer protocol tests exercise both platform origins, a cache file evicted between lookup and read, all-pinned entries, shared cache files, obsolete queued scrubs and cleanup of extracted draft frames. Missing FPS uses the existing 30 FPS default; engine errors remain errors. A local HTTP fixture regression verifies that invalid UTF-8 cannot produce a response or contaminate the next recorded request.

Native thumbnail tests encode owned 64×32 fixtures, verify real 320×160 JPEGs, exercise the short-clip fallback and require unchanged cache timestamps on repeat requests. Audio and invalid inputs must return no thumbnail. Spectator-window creation and reuse run in Tauri's mock runtime. The HTTP fixture accept loop also receives an interrupted accept followed by a real connection, ensuring that the subsequent request is still served.

Windows CI installs Chocolatey's ffmpeg-full package and prepends its actual binary directory to PATH, keeping ffmpeg and ffprobe from the same distribution. The setup step requires libsvtav1 before any export tests run. Gyan's official build inventory lists libsvtav1 among the additional full-build libraries: https://www.gyan.dev/ffmpeg/builds/ . No AV1 test is skipped or replaced.

The timeline benchmark drains MutationObserver records at animation-callback boundaries, recording timer mutations between callbacks separately instead of attributing them to the last edit frame. An owned-node calibration requires one in-frame and one timer mutation in their respective buckets. Every mutation remains counted, and the allowed edit panels, three runs, 10 ms limits, 400-element ceiling and heap limit are unchanged.

The live-feed and chat undo integration tests also require the real engine fixture. Their shared event waiter is tested for timeout on the wrong event, and setup tests require an existing system FFmpeg to satisfy the part without creating a download directory.

A deterministic browser regression reproduces expiry during a pending catch-up frame before the next edit enters the queue. The previous implementation changed the header and missed the scoped repaint. Expiry now leaves that pending frame clean and marks the full refresh deferred, covering both timer/edit orderings without relaxing the DOM mutation contract.

Engine PATH construction uses Rust's platform-aware split/join functions. Tests require exact empty overrides when FFmpeg is already on PATH, and reject a directory that cannot be represented in PATH rather than emitting a malformed search path. Part satisfaction is checked for all three outcomes: downloaded, system and missing.

Export job ownership uses the native process creation time: Linux procfs or Windows GetProcessTimes with a query-only process handle. Windows GetExitCodeProcess distinguishes a running owner from a terminated process; access errors refuse takeover instead of declaring a lock stale. Windows host identity comes from GetComputerNameW. The native-child regression requires a stable current-process identity and checks that an owned child loses its identity after termination and reaping. The existing active-owner, stale-owner, PID-reuse and foreign-host lock assertions remain unchanged.

CLI fixtures also cover malformed model catalogs, incomplete entries and an empty completion following an already received reply. The same event collector checks successful output and verifies that launch/configuration failures emit no events. Platform directory and terminal policies share a single implementation across their injected providers.

The layout-jitter sampler applies its existing deliberate-spring rule to viewer metadata as well as panels and rows. Early sampling reproduced metadata moving with a panel marked data-anim during the initial mode transition. A calibration still requires a real one-pixel metadata shift to measure exactly one pixel; only the explicitly marked spring sample is treated as an animation. The acceptance limit remains 0.5 pixels. Any scenario without a completed measurement now fails the run instead of allowing the remaining measurements to hide it.

Studio acceptance drives the real keyboard handlers for navigation, edits, seeking, zoom and file operations, checking the recorded commands. Native-command failures use owned adapters and must appear in the UI without uncaught browser errors. Telemetry acceptance injects an interrupted accept and closes one socket's write side before greeting, then requires successful delivery on the next real connection.

The desktop MCP entry is exercised in child processes with an owned loopback bridge and a compiled fixture executable. Tests verify attachment, argument forwarding, forced standalone mode, failed execution, missing-engine diagnostics and exit-status preservation. All logs and bridge descriptors stay in the fixture directory.

CLI discovery tests remove an owned cached executable and require rediscovery from PATH or the owned extra-directory location. Directory enumeration failures retain the direct version candidate. Process output readers use the stdout/stderr pipes guaranteed by their own Stdio::piped spawn configuration; they no longer silently accept an internally absent pipe. Missing, null and populated tool errors retain their existing parser results.

A three-turn Studio fixture checks thinking/tool/status events, usage-bearing completion, a reply-only completion and an empty reply. Usage metadata is cleared at the next turn's start: a regression first reproduced the old cost and turn count on the second reply, then verified that the corrected second reply contains neither.

Server handshake regressions inject reader/writer clone failures and timeout-setting failures around real loopback sockets, plus invalid UTF-8 and a closed write side. Every failure must leave both connection notifications and handled requests empty. The successful control must authenticate, return the exact JSON-RPC result and announce both connection and disconnection.

Studio Live acceptance checks sparse external envelopes, unknown actors, error/warning totals and all actor filters. A 610-event sequence must retain exactly 600 entries and render exactly 300 rows, with the oldest fixture event evicted.

The Windows LLVM response-file fallback leaves the `export` subcommand on the command line: llvm-cov dispatches it before expanding response-file options. Every original export option and object remains in the response file. The regression checks the exact subcommand placement, full 500-object inventory, output and failure status, and temporary-file cleanup.

The TCP spectator proofs preserve bytes delivered before Winsock reports ConnectionReset when the sender process exits. Other read errors still fail. Both proofs now also require a final newline, then retain their complete JSON, exact frame ordering, pacing and one-start/one-terminal-per-call assertions; a reset cannot conceal missing or truncated events.

The desktop API command proof uses an owned localhost HTTP server and an in-memory fixture key. It checks the outgoing model/tool-result requests, all translated event kinds and turn IDs, a refused tool call, successful completion and HTTP 401 failure. Both outcomes must release the busy flag.

The desktop command tests also exercise connection reference counts and both exit paths: ordinary shutdown marks recovery clean and removes the bridge descriptor, while update shutdown preserves the recovery record. Local socket fixtures cover bridge clone, timeout, address and relay failures, incomplete greetings, and exact protocol replies. Setup command fixtures resume a verified complete part and cancel a pending local download without starting a subsequent part.

An owned executable fixture drives subscription-agent commands through success, expired-session retry, and terminal failure. Assertions preserve the exact streamed protocol status, selected model/effort, one retry only, retained session ID and release of the busy flag and child slot. No installed subscription CLI or external account is used.

A native failure regression makes a regular file obstruct the save destination, verifies failed saves preserve the remembered project and recovery record, and attempts to restore an absent autosave. Restore retains its offer until the engine succeeds; concurrent Restore/Discard requests serialize on that offer. The regression failed before the fix because the offer was consumed on error.

Cross-platform path regressions preserve rooted forward-slash references instead of rebasing them under the project directory, and compare relative paths with native path joins. The exact error-text golden comparison retains the two Windows missing-parent OS messages through narrowly scoped native suffix expectations; all other bytes and structured error assertions remain unchanged.

The collect-destination golden case owns a uniquely named nonempty temporary directory and verifies its sentinel survives refusal. Only that owned path is represented as the historical `/tmp` label in the complete text comparison; the system temporary directory is never assumed nonempty or collected into. The prior fixture could succeed in an empty container and contaminate later cases.

High-bit exports attach the declared range, primaries, transfer and matrix to the converted encoder frames as well as the stream options. FFmpeg 9.0.2 otherwise drops primaries and transfer from untagged raw input. The existing HEVC/FFV1 tests retain their exact tag and decoded-pixel assertions; the external H.264 fixture also declares its frame metadata explicitly.

Executable entry-point tests launch the actual desktop binary with owned data directories. `--version` must return the package version without creating application state; `--crash-test` must fail and leave exactly one local report with the deliberate panic, version, main thread and backtrace, before creating any engine or bridge state.

Desktop initialization is exercised with the real engine in isolated child processes: valid and malformed project arguments, pending recovery, an unavailable executable, successful autosave and an obstructed autosave destination. Tests pass explicit owned data/configuration directories to the same initialization function used by the window. Updater tests use an ephemeral loopback feed and require an unsigned download to fail before engine shutdown or installation. Native launcher tests capture login/install commands without installing or signing into a real CLI; IPC tests reject malformed required and optional arguments before state changes.

Clip decoding writes every filtered frame exactly once (`-fps_mode passthrough` on the rawvideo output). Its filter chain already conforms the rate with `fps=` and ends with `setpts=PTS-STARTPTS`; since FFmpeg 7, `setpts` no longer forwards the frame rate, so the CLI assumed 25 fps for rawvideo and duplicated frames (`0,0,1,1,2,2,2,…` for a 10 fps source). Because decoded buffers are indexed by frame, FFmpeg 7+ hosts showed wrong frames for time remaps, holds, reverse playback, speed ramps and frame blending, while FFmpeg 6.1 passed. The existing `time_remap`, `speed_ramp_interp`, `kimo_motion_v7` and `quality_v893` assertions reproduce the defect on FFmpeg 7.1.5 and pass with the fix; no expected value changed. The option already applied to stride-2 previews, so the minimum supported FFmpeg is unchanged.

`montage_from_folder_v7` encodes its 4:2:0 H.264 fixtures at 16×10. FFmpeg 7+ refuses the former odd 16×9 size (`height not divisible by 2`) instead of rounding it, which failed fixture creation before kimo was called. Its montage assertions are unchanged.

Linux desktop integration also starts the shipped binary with an owned Xvfb display and D-Bus session, verifies second-instance handoff and closes the native window through its window-manager protocol. The application receives owned XDG directories; the test checks clean recovery state and removal of its bridge file. CI installs `xvfb` and `dbus-x11` for this proof. Existing early-entry tests still verify `--version` and the deliberately generated crash report. These scoped desktop checks do not certify full-workspace coverage or eliminate the separate private-media prerequisite.

The desktop updater test's local feed server keeps its listener non-blocking (so the accept loop can stop), but switches each accepted connection back to blocking before reading the request. On Windows an accepted socket inherits the listener's non-blocking mode, and the read failed with `WouldBlock` (os error 10035); Linux never inherited it. The feed cases and assertions are unchanged.
