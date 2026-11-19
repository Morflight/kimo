# Studio shell and synthetic bridge

Studio exposes Home, Edit, Agent, Live and Connections. First launch presents a welcome screen after dependency setup; dismissing it records onboarding. Connections distinguishes the current project's agents from the optional separate-engine MCP listener. Registering an installed CLI is an explicit action for that agent. API key entry clears after saving and status exposes a hint, never the full secret.

Setup progress uses the native `bytes_per_s` field for each active part’s MB/s display, `done / total` for its progress bar, and marks completed parts installed. Readiness depends on all required parts; optional missing parts do not block startup. The fixture reports `ready` and each part’s `satisfied` state like the native command. The current shell does not display an aggregate ETA. The acceptance test checks these native fields rather than the obsolete aggregate status text.

## Synthetic protocol

`ui/dev/mock-bridge.js` provides an in-memory fixture, not a real engine or credential store. Its default project has caption size 54 and no silence cut. Claude Code starts installed and signed in but unregistered; Codex and Antigravity start absent. Unknown commands and unknown CLI IDs fail. Sign-in and registration require installation. Returned IPC values are detached copies, so consumers cannot mutate fixture state through responses.

`agent_select` accepts known agent IDs or `api`, preserving unrelated settings and trimming optional model/effort values. `select: false` updates preferences without changing the selected agent. API turns require a saved fixture key. `agent_send` receives text and a turn ID, then emits tagged `agent-event` records for thinking, tool start/end and completion. Deterministic caption/cut/color plans simulate edits and emit same-project `live-event`, `changes` and `project-changed` records. They do not call a provider, install software, execute CLI commands or modify a real project.

Changes carry ordered history depths. `change_undo` restores the snapshot before the selected active change and marks it and subsequent active changes undone. Unknown/already undone changes fail. `agent_reset` preserves project history. `agent_stop` invalidates pending synthetic work before a mutation is applied. Separate-engine telemetry uses `external-event` with a connection ID and does not enter the current project's change ledger.

## Acceptance and limits

`mock_bridge_test.mjs` loads the complete module and checks response isolation, command/agent errors, installation prerequisites, selection preservation, key redaction, event tagging, ordered undo, conversation reset, cancellation and separate-engine telemetry. `studio_ui_test.py` exercises welcome, connection controls, explicit registration, key hints, agent edits, viewer revisions, before/after, cached draft playback, outside edits, undo, Live filtering, empty/error/retry and dependency setup in Chromium. Required CI includes light and dark Studio cases.

These fixtures establish UI/protocol behavior only. They do not qualify real CLI authentication, OS keychains, native dialogs, real provider responses, rendering quality or physical audiovisual output. The source remains `legacy-unverified`; a passing browser suite is not full-source coverage or product certification.

Frame stepping retains the paused clock's full numeric precision. Seeking clamps to the timeline but does not round to milliseconds: repeated ±1/fps steps therefore do not accumulate millisecond rounding drift. Display formatting remains separate. A browser regression feeds the clock callback fractional paused positions, then uses the real arrow and K+J keyboard handlers and requires the expected positions within 1e-9 seconds. It fails on the previous rounding behavior. The existing playback acceptance tolerance remains unchanged.

`studio_actions_test.py` exercises manual timeline edits, import, project save/open confirmation, zoom, menus and export errors/completion in Chromium. Its recording native-command adapter checks exact command arguments and supplies synthetic replies; it does not write projects, render an export or certify native engine behavior.

The same proof supplies a synthetic Tauri event envelope and checks update consent, install-error recovery, progress labels and clipboard fallback. It neither installs an update nor uses a native shell.

The actions fixture also checks inspector timing/transform/audio/text/effect arguments, clamped numeric values, project format, new-project consent and model selection. Selecting a model with incompatible effort clears that effort; custom model names are trimmed, while choosing the default model preserves a compatible effort. These are synthetic bridge contracts exercised through real Chromium controls.

`studio_recovery_test.py` verifies startup restore/discard, restoration errors and crash-report consent for one or several logs. Its synthetic native adapter records commands; only the newest log may receive send consent, and no browser report or network submission is opened by the fixture.

The inspector preserves focus only while editing the selected clip. Deleting a clip from its focused inspector button must allow the project properties to render; a Chromium regression checks that transition. Context menus, rename-on-blur, export filename normalization, setup pause/skip and both welcome actions have synthetic command assertions.

Preview preparation controls are tested with a deferred synthetic start response: cancellation must wait for the start, stop the session, clear the pending preparation and display the stopped state. The playback error callback must show its diagnostic.

Fallback image-draft playback advances to the chunk end before loading the next chunk, preventing replay of the same final frame. `studio_draft_test.py` uses two synthetic chunks and requires exactly one next-draft request and a final stop. Escape closes an open overlay/menu first; a subsequent Escape in Edit clears the selection without interfering with text entry.

The native-adapter browser proof distinguishes system, downloaded, optional pinned and unavailable setup parts. It asserts progress percentages, speed clearing on completion, error text, retry availability, cancellation dispatch and ready-state dismissal, without performing real downloads.

Each agent turn starts with empty usage metadata and no previous error. A reply-only completion must not inherit the previous turn's cost or turn count. The three-turn browser regression checks thinking truncation, tool/status events, usage-bearing completion, reply-only completion and an empty reply, including completion while Edit is visible.

The action proof also exercises keyboard navigation and editing commands, exact seeking, failed or cancelled native file dialogs, connection command failures and Live actor filters. A 610-event fixture verifies the existing 600-entry history limit and 300-row rendering limit, including eviction of the oldest event. These cases retain the synthetic-adapter limits above.

Additional browser cases verify proxy diagnostics and state retention while the media search hides an asset. Connection fixtures check session-only and remembered-key receipts and both explicit and missing MCP probe errors. Router assertions cover partial read replies, private-ID redaction, fallback call timing, bounded camera/mesh history, tracking residual validation, and geometry retained by a layout whose panels are not yet available.

Export browser cases retain the empty-timeline guard, cancellation without a path, refusal without a job ID, missing progress counters, completion using the requested output path, and warning/cancellation diagnostics. They use synthetic jobs and perform no export.

Media tests generate thumbnails in memory and verify insertion, cached filtering, fallback command diagnostics, rejection of a non-project read response and two-file import from Home into Edit.

Starting a new conversation releases the cancelled turn immediately and clears its status, even if the native agent promise settles later. A deterministic deferred-adapter regression covers both late success and late failure, verifies neither can replace the new conversation, and retains the busy guard against duplicate sends. The regression reproduced a stuck busy state before the fix.
