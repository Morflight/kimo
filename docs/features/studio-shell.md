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
