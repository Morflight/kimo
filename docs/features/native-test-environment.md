# Native test environment

The complete native suite executes on Linux and Windows, including explicitly ignored proofs. Missing inputs, failed subprocesses and unmeasured files remain failures. Native tests currently mutate process-wide environment and therefore run serially in coverage CI; this does not change their assertions or thresholds.

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
