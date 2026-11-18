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
