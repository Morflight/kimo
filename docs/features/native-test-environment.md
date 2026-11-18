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
