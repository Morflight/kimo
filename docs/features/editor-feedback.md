# Editor feedback: transient flashes

## Purpose and lifetime

The spectator editor marks changed rows, clips and other affected panels with flash timestamps. The router's `flashing(timestamp)` treats a non-null stamp as active for **700 ms**. `freezeNow` supplies one time reading during a render pass; outside a frozen pass the live performance clock is used. `takeFlashEnd` returns the earliest active expiry observed during rendering and resets that accumulator.

`app.js` arms one timeout for that earliest displayed expiry. Its callback paints directly, removes expired flash classes and schedules the next still-visible expiry. The idle editor must not retain a stale `.flash` class indefinitely or start an animation-frame polling loop just to remove it. Hiding the editor, leaving the page and tearing down the affected panel clear the pending expiry timer. Flash feedback conveys a recent change; it is not durable history or proof that an engine operation succeeded.

## Acceptance and test timing

`flash_expiry_row_test.py` checks the full lifetime, the 680 ms pre-expiry boundary, post-expiry DOM cleanup, the historical 735 ms stale-flash case, three overlapping stamps, direct repaint counts, subsequent idle behavior and pagehide/panel/visibility teardown. `flash_stamps_test.py` complements it with state/timestamp behavior.

The overlap fixture schedules its three events at 250 ms intervals inside the browser. Setup, stamps and deadline snapshots stay in the same JavaScript task sequence so Python/CDP round trips cannot accidentally push the third event beyond the first stamp's 700 ms lifetime. Production duration and assertions remain unchanged. Reads that miss the next expiry window now fail explicitly rather than being treated as unjudged passes. Test frame waits are separate from the app's own animation-frame metric.

This remains a focused contract within the legacy editor area. A passing synthetic Chromium proof does not qualify all editor rendering, browser platforms, native presentation or frame performance.
