# OpenColorIO reference bridge

## Purpose and dependencies

`crates/kimo-core/src/color/ocio_bridge.py` is the optional CPU reference processor used by `color/ocio.rs`. It requires **OpenColorIO 2.5.1 exactly**; it does not install dependencies or silently choose another version. Missing PyOpenColorIO, a version mismatch or an unsupported view terminates with an explanatory nonzero exit.

The caller passes width, height and mode as positional arguments. Dimensions are integer pixel counts supplied by the Rust caller. The script does not independently enforce positive dimensions or resource budgets; zero-size input currently terminates without emitting a frame. The Rust integration and its input validation remain separate qualification requirements.

## Color and streaming contract

The built-in config is `cg-config-v4.0.0_aces-v2.0_ocio-v2.5`, the source color space is ACEScg, and processing uses the default CPU processor for a DisplayViewTransform.

| Mode | Display | View |
| --- | --- | --- |
| `sdr` | sRGB - Display | ACES 2.0 - SDR 100 nits (Rec.709) |
| `pq2020` | Rec.2100-PQ - Display | ACES 2.0 - HDR 1000 nits (Rec.2020) |

Stdin/stdout carry binary RGBA float32, little-endian, 16 bytes per pixel. Each complete `width × height` frame is processed through a four-channel PackedImageDesc, written and flushed before reading the next frame. Short reads are accumulated; EOF between frames succeeds without extra output. EOF within a frame raises `truncated input frame` and does not process or emit that incomplete frame. Previously completed frames remain available. Native big-endian hosts swap input before processing and swap output back to little-endian.

Processor/configuration/I/O errors are not converted into successful output. The caller must check exit status as well as byte counts. The bridge does not itself implement cancellation or a processing timeout. Physical display characteristics and export encoding are outside this binary protocol.

## Acceptance and evidence

`tools/quality/test_ocio_bridge.py` checks both exact display/view selections, version and dependency failures, unsupported modes, multiple frames, partial reads, truncated frames, clean EOF and little-/simulated-big-endian conversion. The injected processor deliberately changes a channel so that output assertions prove processed bytes were emitted. OpenColorIO itself is a double: these tests do not certify transform accuracy, alpha semantics of the real processor or real big-endian hardware.

The optional native reference/export tests and actual OpenColorIO fixtures still require execution. The catalog therefore retains `legacy-unverified`. Formatting the Python bridge as separate statements exposes conditional paths to branch measurement; it does not change its streaming or color behavior.
