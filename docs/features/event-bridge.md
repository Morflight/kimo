# Editor event transport

## Purpose and selection

`crates/kimo-gui/ui/bridge.js` sends incoming events to one `onEvent` callback and connection state to `onStatus(kind, label)`. It owns transport selection, not event reduction or project edits. `connect` returns a transport descriptor; it does not expose a disconnect method or promise teardown on navigation.

| Environment/query | Selection and initial status |
| --- | --- |
| `window.__TAURI__` present | Tauri always wins, even with `feed=sim` or `feed=off`; initial `down/listening` |
| Browser `feed=off` | No transport; `down/no feed` |
| Browser `feed=sim` | Demo replay, visibly labelled `sim/simulated` |
| Browser otherwise | SSE at `http://127.0.0.1:7422/events`; `sse=PORT` overrides the port |

The browser URL parameter is a trusted development input, not a validated arbitrary endpoint API. The implementation uses loopback rather than an external server.

## Liveness and delivery

Tauri subscribes to `engine-event` and `spectator-status`. The first engine event establishes `live/live`, even if the shell's one-time live status arrived before listeners registered. Further events are forwarded without redundant live transitions. A subsequent listening status becomes `down/reconnecting` after a live connection has been seen; before the first live connection it retains its literal label. Other status payloads are converted to strings. A later engine event or live status restores liveness.

Browser SSE forwards parsed JSON. Malformed JSON is ignored; exceptions thrown by the event callback are also swallowed by that same handler. The most recent nonempty SSE event ID is remembered, including on a malformed message, and URL-encoded as `?last=...` when explicitly reopening. The transport does not validate the event schema.

Before any successful open, an SSE error closes the stream. Forced `feed=sse` reports `down/sse :PORT refused` and does not switch to simulated data or schedule a retry. Automatic mode falls back to explicitly labelled demo replay. If the development-only replay module cannot load or its `play` call throws, status becomes `down/no feed`.

After at least one open, errors report `down/reconnecting`. EventSource handles retries itself while it remains CONNECTING. If it becomes CLOSED, the bridge schedules a new stream after 500 ms, doubling on successive failures to a maximum 8 seconds. A successful open resets the delay to 500 ms. An idle/open connection has no bridge retry timer. The API does not currently guard duplicate CLOSED error callbacks or provide listener disposal; those are lifecycle limitations, not promised behavior.

## Recovery limits

SSE replay depends on the development feed's event-ID ring. Tauri's engine stream has no replay buffer: events missed while disconnected cannot be reconstructed by this module. UI handling of incomplete calls belongs to the router/app, not transport delivery. A live indicator proves connectivity, not complete history or successful editing.

## Acceptance and evidence

`crates/kimo-gui/tests/bridge_test.mjs` exercises the complete original ES module with transport/replay doubles and unchanged source offsets. Eight cases check selection precedence, liveness transitions, payload delivery, malformed JSON, encoded replay IDs, native-versus-explicit retries, capped/reset backoff, initial refusal and replay availability. Run `bash tools/fork/test-ui.sh` under Node 22 to include these plus all six preview suites.

The existing `reconnect_test.py` provides real Chromium/loopback-feed checks, including refusal and no-replay variants. The no-replay receipt explicitly distinguishes heard calls from calls unavailable in the gap; it must not claim lossless delivery. Native Tauri listener delivery, lifecycle cleanup and supported-platform coverage still require qualification; this focused contract remains `legacy-unverified`.
