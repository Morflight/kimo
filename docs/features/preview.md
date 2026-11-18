# Preview and background preparation

## Purpose and controls

Edit mode displays the current frame, schedules preparation after one idle second, and exposes Prepare preview, Cancel preparation and Clear prepared preview. Play can reuse the prepared session at the same playhead. Manual cancellation remains sticky until explicitly restarted. Editing, scrubbing, hiding or leaving Edit cancels preparation.

## Playback contract

The startup reserve remains 32 frames. A short final tail can start without a full reserve. Buffer underruns retain timeline progress rather than implicitly rewinding. Presentation follows the audio clock. Normal preview cadence targets about 30 fps and is never configured below 27 fps unless the timeline itself is slower. Export and dense temporal paths keep their full-frame behavior.

Frame fetch failures, HTTP/decode errors and persistent missing frames end buffering with an explicit error after bounded waiting. Transient missing frames can recover. Pausing cancels pending timeouts; responses from replaced generations cannot corrupt the new playback session. Audio delivery failures are surfaced once per generation.

## Background budget and fallback

Automatic preparation allows plain projects up to 1920×1080 pixels, source assets up to 3840×2160 pixels and at most 256 clips. Ineligible projects skip preparation but retain ordinary playback. It prepares at most 32 frames using one rendering worker, single-thread FFmpeg codecs/filters, no speculative prefetch, a 64 MiB decode-cache budget, 20 ms pauses and an independent five-second watchdog. The cache budget is not a total-process RAM limit. Windows render/decoder background priorities reduce competition. Audio decoding waits for Play.

## Acceptance cases and evidence

- `playback_delivery_test.mjs`: fetch/HTTP/decode failures, bounded waiting, recovery, pause and stale generations.
- `playback_sync_test.mjs`: audio-driven picture, chunk continuity, low-fps timeline, shuttle/loop and stride behavior.
- `playback_buffer_test.mjs`: reserve, jitter and short tails.
- `playback_rebuffer_test.mjs`: underrun recovery without implicit rewind.
- `playback_prepare_test.mjs`: in-flight cancellation, reuse, release, lag, eligibility rejection and changed playhead.
- `prepare_ui_test.mjs`: idle scheduling, revisions, sticky cancellation, visibility and navigation.
- Native `preview_stream.rs` integration tests and `profile_preview.rs` comparisons exercise engine lifecycle/render behavior. They are not substitutes for Windows/Linux measured coverage or desktop acceptance.

The six controller suites passed during fork setup. Prior Windows measurements and their limitations are recorded in `docs/FORK.md`. Full native coverage and physical sound/canvas qualification are outstanding. No claim of 100% coverage or universal smooth playback is made.

## Browser controller API and resource lifecycle

`ui/playback.js` defines `KimoAV` and `KimoPlayer` as a classic script. The player receives engine `invoke`, a canvas, `fps`, `duration`, `scale` and `onTime` callbacks; `onState`, `onError` and `onWarm` are optional notifications. Falsy FPS defaults to 30, falsy duration to zero, and falsy Play rate to 1. Engine preview commands are `preview_start`, `preview_update`, `preview_frame` and `preview_stop`.

`ensure(time)` starts a session once. Recognized unknown-command/not-allowed/preview-stream errors mark streaming unsupported and return false, allowing the editor's ordinary draft path; other failures notify `onError` and remain retryable. Polls normally avoid overlap, but explicit seek/state updates may proceed. A `no preview stream` update error clears the started state and attempts a fresh session. Other errors are reported; polling state is released even on failure.

Session generations invalidate cached frames and clear delivery-error state. Frames download with `cache: no-store`, at most eight in flight. HTTP failures are rejected before bitmap decoding. A bitmap decoded after its generation changed is closed without publication. ImageBitmap is preferred; the fallback uses an HTML Image. The decoded-frame cache retains up to 60 entries, prioritizing nearby frames in the run's direction and discarding frames well behind the playhead. Eviction and explicit cleanup close available bitmap handles. The limit describes decoded frame count, not process memory.

Playback updates the engine every 120 ms. Pause clears playback clocks, animation/poll timers and scheduled audio sources, reports `paused`, and tells an existing engine session to stop playing. Stop also cancels preparation, releases the engine session and clears decoded caches. Cleanup tolerates already-stopped audio sources and an engine that rejects stop. Changing rate to zero pauses; another rate starts/restarts from the current playhead.

Scrubbing uses a cached exact frame when available. Otherwise it permits one `preview_frame` request in flight and retains only the latest requested next time. A superseded response is closed without drawing; the latest queued request follows. A playback epoch prevents an older scrub from painting over a newly started Play session. The canvas adopts the decoded dimensions and `onState('scrub', ...)` reports elapsed time, engine render time and cache status. A missing session clears started state; other scrub errors notify the error callback. Browser decoding and presentation still need real-browser qualification.

## Audio timing and endpoints

Normal forward rate 1 can use AudioContext (or webkitAudioContext), requested with 48 kHz and playback latency. An engine `audio_on: false`, another playback rate or unavailable audio API selects the performance clock. Play attempts to resume a suspended audio context; a rejected resume is tolerated. A running audio device must have a moving clock before buffered playback starts. Initial chunk waiting is bounded at 1.5 seconds, while the separate total buffering deadline remains 15 seconds.

Chunks are decoded once by name, scheduled only while playing with an active clock, and aligned to their timeline interval. Chunks already entirely behind the clock are skipped; partially elapsed chunks start at an offset. Silent/unloaded/already-scheduled chunks are not scheduled. Ended sources relinquish their references. Once the chunk map exceeds 24 entries, old chunks ending more than two seconds before the playhead are pruned when they have no source. Thus 24 is a pruning trigger, not a hard cache maximum.

Clock anchoring includes device output latency, falling back to base latency, and 50 ms scheduling lead. Forward completion stops at the final timeline frame; reverse completion stops at zero. An enabled loop wraps forward to its in point and reverse to one frame before its out point (clamped to the timeline end). Missing pictures hold position initially; after 300 ms the controller buffers again without rewinding the clock.

`playback_lifecycle_test.mjs` adds success/error/lifetime assertions for session fallback/recovery, cleanup, frame eviction, stale generations, audio scheduling and resume errors, polling, coalesced scrubs, epochs, endpoint/loop handling and audio startup. These use deterministic transport/audio/bitmap doubles. They complement the existing preview tests and do not certify audible output or render quality.

The native `color_cache.rs` fixtures explicitly distinguish dense stride-1 windows from sparse stride-2 windows. Sparse lookup uses source frame numbers, floors between sampled slots and clamps beyond the window; coverage checks reject unsampled endpoints. Ordinal indexing still selects the corresponding decoded slot. These tests protect the frame-stride representation shared with preview decoding; their passing native execution must be recorded separately from controller tests.

## Passive spectator versus active editing

The spectator (`ui/index.html`) consumes engine telemetry and supplied frame pixels. A project revision marks an older picture out of date until the matching/newer frame arrives; out-of-order frames cannot overwrite a newer displayed revision. Legacy frames without revision metadata remain displayable. Telemetry replay must not cause this passive spectator to invoke native rendering.

The active editor (`studio.html`) intentionally requests viewer/preview frames through its native bridge. The inherited revision proof's blanket ban on the text `render_frame` in every GUI Rust file predated this active viewer and also matched prompts and read-only tool classifications. The revised proof retains all revision, stale-frame, identity and read-only assertions; it adds a rejecting/recording native bridge to all three spectator scenarios and requires zero invocations. The UI direct-tool-call tripwire remains. Native viewer rendering is tested separately and is not banned by the passive-spectator contract.
