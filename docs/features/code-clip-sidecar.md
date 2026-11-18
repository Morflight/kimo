# Code clip rendering sidecar

## Purpose and boundary

`crates/kimo-core/src/codeclip_sidecar.py` implements the Python/browser portion of code clips. It renders a local HTML project into straight-alpha RGBA8 frames. The Rust caller in `codeclip.rs` owns validation, timeline sampling, cache keys, process deadlines and process cleanup; this focused contract does not certify those caller behaviors. Code clips use `window.renderFrame(t, params)` rather than wall-clock animation.

## Job input and defaults

Invoke `python3 codeclip_sidecar.py job.json`. The job requires `dir`, `width`, `height` and an ordered `steps` list. Dimensions are converted to integers. The trusted Rust caller supplies valid positive dimensions and output paths; this script is not an independently validated public job API.

| Field | Behavior |
| --- | --- |
| `entry` | Default `index.html` when absent or empty; URL-encoded for navigation |
| `frame_timeout_ms` | Default 20,000 when absent or zero; used for async render promises and screenshots |
| `load_timeout_ms` | Default 60,000 when absent or zero; used for navigation and readiness |
| `chromium` | Explicit executable, otherwise search the Playwright cache; a nonexistent executable falls back to Playwright's default |
| `lock` | Optional process-lifetime exclusive file lock; Windows retries busy locks after 0.5 seconds |
| Frame step | `t` is converted to float for JavaScript; absent/empty `params` becomes `{}`; `out` is the raw output path |
| Reload step | Truthy `reload` reloads the entry page without emitting or numbering a frame |

Discovery checks `PLAYWRIGHT_BROWSERS_PATH` before the platform cache. Headless-shell patterns precede full Chromium patterns; matching paths are reverse-lexically sorted and must be files. This is not a semantic version comparison. Cache locations differ on Windows, macOS and Linux.

## Frame and process protocol

The context uses the requested viewport, device scale 1, JavaScript enabled and service workers blocked. Readiness waits for `__ready === true` or, when `__ready` is undefined, a callable `renderFrame`. A callable renderer is required even after `__ready` becomes true. Font readiness is awaited before rendering.

Each frame clears prior page errors, calls the renderer with time/parameters, waits for returned promises, checks new page/console errors and captures a transparent PNG. Pillow converts it to RGBA; a dimension mismatch fails before replacing an existing output. A frame is written to `out + '.part'` then atomically renamed to `out`. Job/asset reads close their file handles after reading. The output is row-major RGBA8, four bytes per pixel.

Stdout is newline-delimited JSON: each successful frame reports zero-based `frame`, the original `t`, and elapsed `ms`; final success reports `ok`, `frames`, `blocked` requests and the WebGL renderer string. Reload steps do not consume frame numbers. An empty steps list succeeds with zero frames. An exception emits an `error` string limited to four lines/900 characters and exits 3. Missing Playwright/Pillow produces installation guidance and exits 3. Previously published frames can remain when a later frame fails; success must be determined from both the process exit and final response.

## Request isolation and limits

Only `http://kimo-clip.invalid/` maps to the clip directory. Paths are URL-decoded and resolved through symlinks; missing files or paths escaping the directory return 404. Other origins are aborted. JavaScript modules use `text/javascript`; other assets use guessed MIME types or `application/octet-stream`. Diagnostics retain at most 20 blocked URLs, each truncated to 200 characters.

The initialization script freezes Date/performance time, seeds Math.random per page load and replaces WebSocket, RTCPeerConnection and EventSource with throwing functions. Chromium also receives DNS/background-network restrictions. These mechanisms do not constitute a security audit or an OS sandbox guarantee. The job's `out` path is trusted and is not constrained to the clip directory.

The promise timeout cannot interrupt a synchronous infinite JavaScript loop. The Rust whole-process deadline is therefore essential. Browser exceptions rely on Playwright/process teardown; this script does not promise cancellation latency or remove all partial outputs after I/O failures.

## Acceptance and evidence

`tools/quality/test_codeclip_sidecar.py` checks atomic byte publication, frame numbering, parameter/default propagation, reloads, readiness failures, console errors, wrong dimensions, error exit status, dependency guidance, request containment including symlinks/traversal, diagnostic bounds, lock retry and platform discovery. Browser and Pillow doubles deliberately isolate protocol behavior; they cannot establish pixels, actual network isolation, native Windows locking, WebGL or asynchronous JavaScript semantics.

Real Chromium integration suites already exist in `crates/kimo-core/tests/code_clip.rs`, `code_clip_determinism.rs` and `code_clip_batches.rs`. Their current supported-platform execution and coverage remain required. This feature remains `legacy-unverified` pending native/browser qualification, despite the new protocol tests.
