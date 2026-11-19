# Editor feedback: transient flashes

## Purpose and lifetime

The spectator editor marks changed rows, clips and other affected panels with flash timestamps. The router's `flashing(timestamp)` treats a non-null stamp as active for **700 ms**. `freezeNow` supplies one time reading during a render pass; outside a frozen pass the live performance clock is used. `takeFlashEnd` returns the earliest active expiry observed during rendering and resets that accumulator.

`app.js` arms one timeout for that earliest displayed expiry. Its callback paints directly, removes expired flash classes and schedules the next still-visible expiry. The idle editor must not retain a stale `.flash` class indefinitely or start an animation-frame polling loop just to remove it. Hiding the editor, leaving the page and tearing down the affected panel clear the pending expiry timer. Flash feedback conveys a recent change; it is not durable history or proof that an engine operation succeeded.

## Acceptance and test timing

`flash_expiry_row_test.py` checks the full lifetime, the 680 ms pre-expiry boundary, post-expiry DOM cleanup, the historical 735 ms stale-flash case, three overlapping stamps, direct repaint counts, subsequent idle behavior and pagehide/panel/visibility teardown. `flash_stamps_test.py` complements it with state/timestamp behavior.

The overlap fixture schedules its three events at 250 ms intervals inside the browser. Setup, stamps and deadline snapshots stay in the same JavaScript task sequence so Python/CDP round trips cannot accidentally push the third event beyond the first stamp's 700 ms lifetime. Production duration and assertions remain unchanged. Reads that miss the next expiry window now fail explicitly rather than being treated as unjudged passes. Test frame waits are separate from the app's own animation-frame metric.

This remains a focused contract within the legacy editor area. A passing synthetic Chromium proof does not qualify all editor rendering, browser platforms, native presentation or frame performance.

## Panel transition timing

`spring_test.py` requires panel transitions to settle within 200 ms of the mode event, with at most 0.5 px overshoot/still-node movement, 16.7 ms frame intervals plus the existing 0.5 ms clock tolerance, and no lingering app animation frames or newly created timers. The timing origin is captured in the same browser call that injects the event. Starting from the observer's first sample before a separate Python/CDP event call incorrectly included transport delay in the spring budget. The corrected proof retains every performance limit and rejects samples predating the event. These are measured local-browser limits, not a certification of every workstation.

## Timeline performance proof

`timeline_perf_test.py` measures 1,000 clips over three fresh-page edit runs, with 240 split/move events per run. Event injection and frame collection run in a single browser call at the existing 34 ms cadence, avoiding thousands of Python/CDP round trips. It retains per-frame p95/max <10 ms edit-work assertions, mutation attribution, <=400 DOM elements, names/seams, heap growth, pan and scrub measurements. The enclosing runner deadline is not a performance allowance; reported browser timings remain separate from transport/setup time.

## Interrupted calls

A reconnecting status enters the same ordered queue as tool events. When a subsequent tool start proves the serial engine moved on, any still-running earlier call becomes `lost` with unknown duration. A replayed result can settle it normally. Queued starts from before the rupture must be reduced before the gap marker; comparing their paint-time timestamps to a transport-time timestamp could otherwise leave an interrupted call running forever. The deterministic `feed_gap_test.py` covers this ordering, and the no-replay reconnect proof checks actual EventSource delivery. Calls entirely missed during a gap cannot be reconstructed.

A flash expiry arriving while a clip-only frame is queued preserves that frame's timeline/rail-only paint. Other expired panels are refreshed by the following deferred frame. `flash_edit_scope_test.py` holds and then delivers a real expiry callback at this interleaving, verifies the scoped repaint counter and unchanged header during the edit, then verifies the header catches up. The full timeline performance limits remain unchanged.

## Graph domains and curve history

Graph operations accept only finite key coordinates for display. If the time span or its ruler padding overflows, or rounding prevents a nonzero span, the graph clears its ruler, curves, lane labels and playhead rather than writing NaN/infinite drawing coordinates. Inspector measurements remain available. Normal key movement, smoothing, reversal, scaling, copy counts and undo/redo snapshots retain their existing values. Curve-copy operations initialize their cache once and reuse it; every internally constructed graph lane supplies its current key array explicitly.

`cov_b_unit_test.mjs` exercises complete modules and the real router helper bundle. It asserts sparse inspector rows, original and moved keys, reverse/invert transformations, dense-key drawing limits, missing measurements and bounded undo/redo. Two huge equal times and spans overflowing either arithmetic or padding must leave no ruler or plotted curve. The oversized-span regression fails against the earlier code, which produced `first: NaN` and infinite endpoints; no output threshold is loosened.

The timeline controller normalizes the track list once before synchronization; an empty timeline produces zero moved clips. Complete-source tests in `crates/kimo-gui/tests/cov_a_unit_test.mjs` check sparse arguments, capped markers, pulse/swing/kick/flash curves, all ripple-cut boundaries, multicam span merging, missing tracks and undo alignment without source or counter exclusions.

The Chromium interaction proof checks pointer-anchored timeline zoom, horizontal/shift-wheel pan, line/page wheel deltas, bounded track scrolling, picked-call overflow and Escape dismissal. A synthetic shell-window adapter verifies that minimized/invisible windows suspend renders and resume pending updates when visible; it does not claim a native window-manager integration proof.

Complete-source reducer tests assert nested merges, the 400-call history limit, lost-start recovery, scope-frame isolation, stale-preview rejection and the 2,048-event queue policy (old progress first, then oldest event). Superseded image decoding must not publish a parade. These state tests use inert DOM/timer adapters; the separate Chromium interaction proof exercises real layout and rendering.

Wheel input and deferred panel refresh ignore a stale or incomplete mode definition without throwing. The reducer test exercises both missing modes and missing panels.

Complete-source panel tests verify distortion-grid coordinates, signed 3D orbit arcs, deterministic waveforms, camera-solve point counts, scope image labels, dense timeline seams and compact modelling/light layouts. These assertions supplement the real-browser rendering proofs.

The measured light-rig layout (`opts.cx` set) renders before the engine reports any light: absent `lights` and `rows` lists still draw the ring, the subject and the LIGHT/TEMP/INTENSITY/FALLOFF header, with no light rays and no row values. `panels_unit_test.mjs` asserts this empty-rig state alongside the populated measured defaults.

While a grade or effect is being applied, the viewer wipes across the frame: with a source and `wipe` below 1, the ungraded frame stays visible right of the edge (`clip-path` inset at `wipe × 100 %`) and the edge sits at `wipe ×` frame width. A finished (`wipe` 1) or absent wipe shows only the result. The unit test asserts both states deterministically; browser runs only reach the intermediate state when a capture happens to land mid-animation, so they cannot be relied on for it.

An inspector widget’s `gap` sets its internal label spacing; it does not turn the widget into an empty spacer. A block with only `gap` still produces a spacer. The compressor test supplies `gap: 10` and requires the widget markup and threshold label.

The graphics inspector retains the eight most recent shape-layer names and computes its selection after trimming that list, so the latest layer stays highlighted. Router operation tests assert cached arguments, normalized geometry, style updates, invalid inputs and the selected index after twelve inserted layers.

Diff rows retain readable default text when their marker has no configured colour. Both the marker and its label use explicit theme fallbacks; the renderer never emits `color:undefined`. Unit coverage includes an unrecognised marker and partial style maps.

`panels_canvas_test.py` checks real Chromium pixels for CIE opacity and geometry, identical cached repaints, waveform/vector changes and distinct false-colour exposures across cache eviction. It uses in-memory canvases and synthetic images only.
