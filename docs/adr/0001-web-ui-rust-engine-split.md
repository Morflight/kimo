# ADR 0001: JavaScript interface, Rust engine, desktop and SaaS delivery

- Status: accepted (desktop interface technology still open, see below)
- Date: 2026-11-19
- Decider: repository owner

## Context

Kimo is a video editor. Its desktop application (`crates/kimo-gui`) is a Tauri 2 app: a Rust process plus an interface rendered in the system webview from `crates/kimo-gui/ui` (`"frontendDist": "ui"` in `tauri.conf.json`). All visible UI is HTML/CSS/JavaScript (about 7,000 lines: Studio, spectator, timeline, panels, playback). Decoding, rendering, color, analysis and export run in Rust (`kimo-core`, served by the `kimo-mcp` engine).

The question was whether the interface should move to a native Rust UI toolkit (egui, iced, Slint, gpui) for performance. A webview UI is cheaper to build and test for dense layouts, while a native UI avoids transferring preview frames from the engine to the webview.

## Decision

1. **The interface stays in JavaScript**, so the same UI can be delivered in a browser if Kimo becomes a SaaS product.
2. **All heavy lifting stays in Rust**: decoding, rendering, compositing, color, audio, analysis and export. The UI displays results and sends user intent; it does not process media.
3. **Delivery follows the usual pattern of serious web services**: the web UI is the default entry point of a SaaS offering, and editors with capable computers install the desktop application to use their own hardware for better performance.
4. **When rendering becomes too expensive for the user's machine** (web or desktop), the product can provide compute on remote instances running the same Rust engine.

### Open question: desktop interface technology (TBD)

Whether the desktop application keeps the JavaScript interface in a webview (current Tauri app) or gets a native Rust interface is **not decided**. This ADR fixes the JavaScript interface for the web/SaaS delivery and the Rust engine for both; the desktop interface will be decided in a later ADR. Options on the table:

- keep the shared JavaScript interface on desktop (one UI codebase for web and desktop);
- keep it but draw the preview viewer natively on the GPU (e.g. wgpu), where frame transfer to the webview is the main cost;
- a native Rust desktop interface (egui, iced, Slint, gpui), maximizing local performance at the cost of a second UI codebase.

Until then, the current Tauri desktop app remains the shipped desktop interface and stays under the quality contract.

## Consequences

- **The engine boundary is the product's architecture.** The UI must reach the engine only through a transport-neutral contract, so the same UI can talk to a local engine (desktop) or a remote one (SaaS, remote render). Today this boundary partly exists: `studio.js` goes through a bridge object `B` (`invoke`, `listen`) that browser tests replace with `ui/dev/mock-bridge.js`, and the spectator window (`bridge.js`) already accepts events over Tauri or SSE. Studio commands are still Tauri IPC names and arguments; a web build needs an HTTP/WebSocket implementation of the same `B` contract.
- **Desktop-only capabilities must stay behind that boundary**: native file dialogs, local paths, OS keychain, installed CLIs, crash reports and updates. A web build replaces them with uploads, server-side storage and account-based credentials rather than reaching the file system directly.
- **Media transport becomes a design concern**: preview frames and uploaded media cross the network in SaaS mode. The existing bounded preview pipeline (cadence, startup reserve, rebuffering) is the basis to extend to remote engines.
- **Testing follows the split**: Rust tests own engine behavior; Node and Chromium tests own UI behavior through the bridge contract. Both remain under the 100% coverage requirement of `docs/QUALITY.md`.
- **Cross-webview behavior must be qualified**: desktop ships on WebView2 (Windows) and WebKitGTK (Linux), and a SaaS would add mainstream browsers. Browser acceptance currently runs Chromium only.

## Alternatives considered

- **Native Rust UI as the only interface (egui, iced, Slint, gpui)**: rejected for the product as a whole, because it gives no path to browser delivery. It remains an option for the desktop application alone (see the open question): best local preview performance with no frame transfer, at the cost of a second UI codebase and its tests, and weaker accessibility and UI-testing tooling.
- **Electron instead of Tauri**: same web UI with a bundled Chromium (uniform rendering), at the cost of larger downloads and memory use. Not needed while the system webviews are adequate.
