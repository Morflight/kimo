# Feature documentation

Start with the capability map below. `FEATURES.md` is the detailed inherited reference, including parameters, limits and historical evidence. The fork's current playback contract is [preview.md](preview.md). [QUALITY.md](../QUALITY.md) defines mandatory tests and coverage; a listed capability is not a claim that current qualification has passed.

| Area | User-visible capability | Detailed contract / limitations |
| --- | --- | --- |
| Timeline | Import, trim/split/move clips, tracks, keyframes, markers, undo/history | `FEATURES.md`: Timeline and editing; real projects must be preserved |
| Preview | Scrub, play/shuttle, audio scheduling, prepared previews | [Preview contract](preview.md); physical output not certified by controller tests |
| Proxies | Generate preview media and preserve original export sources | `docs/PROXY_WORKFLOW.md`; format/opacity constraints apply |
| Export | Render video/audio and track export jobs | `FEATURES.md`: export/delivery sections; output validation required |
| Color | Grading, curves, LUTs, managed color and scopes | `FEATURES.md`: Color and grading; beta limits are part of the contract |
| Audio | Mixing, gain, automation and analysis | `FEATURES.md`: Audio; simulated scheduling is not an audible-quality test |
| Text | Titles, subtitles, captions and layout | `FEATURES.md`: Text and titles; font availability affects output |
| Composition | Layers, masks, transforms, motion and transitions | `FEATURES.md`: Compositing and motion / Transitions |
| Tracking | Tracking, roto, stabilization and related masks | `FEATURES.md`: Tracking and rotoscope; retain beta/footage caveats |
| Effects / 3D | Particles, graphs, materials, lighting and geometry | `FEATURES.md`: corresponding effect sections; many paths need native/feature-specific tests |
| Automation | MCP tool discovery/execution and CLI commands | `FEATURES.md`: Use from an MCP client / starter tools |
| Desktop | Project interaction, native dialogs, engine connection and updates | `docs/FORK.md`: fork identity and engine/update limitations |

## Focused runtime contracts

- [Studio shell and synthetic bridge](studio-shell.md): onboarding, connections, agent events, change history and fixture limits.

- [Editor transient feedback](editor-feedback.md): flash lifetime, expiry repaint and teardown.

- [Editor event transport](event-bridge.md): Tauri/SSE selection, liveness, reconnect backoff and replay limits.

- [Code clip rendering sidecar](code-clip-sidecar.md): job defaults, frame output, local request routing, errors and browser qualification limits.
- [OpenColorIO reference bridge](ocio-reference-bridge.md): pinned color configuration, binary streaming, failures and reference-library qualification limits.

These own the Python helpers separately from the broad core entry. They do not certify their Rust callers.

## Coherence rules

`catalog.json` inventories every first-party Rust/Python/JavaScript runtime source under crates and maps ownership to documentation and tests. The checker rejects missing files, unclassified new sources, duplicate ownership, and implementation changes without simultaneous updates to linked docs and tests. The initial catalog is deliberately marked `legacy-unverified`: broad inherited links are a starting inventory, not proof that every behavior is documented or tested. Split areas into focused feature contracts as work touches them.

A feature contract must specify purpose, inputs/defaults, observable success behavior, error/cancel behavior, limits, acceptance cases and evidence status. Test assertions must establish those promises. Changes to assertions, coverage scope and contract meaning require review; passing linkage alone is insufficient.
