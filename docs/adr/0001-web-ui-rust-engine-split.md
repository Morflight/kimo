# ADR 0001: JavaScript interface, Rust engine, desktop and SaaS delivery

- Status: accepted
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

No native Rust UI rewrite is planned. If preview playback becomes the limiting factor on desktop, the preferred remedy is a native GPU surface for the viewer only (e.g. wgpu), with the webview keeping the rest of the interface.

## Consequences

- **The engine boundary is the product's architecture.** The UI must reach the engine only through a transport-neutral contract, so the same UI can talk to a local engine (desktop) or a remote one (SaaS, remote render). Today this boundary partly exists: `studio.js` goes through a bridge object `B` (`invoke`, `listen`) that browser tests replace with `ui/dev/mock-bridge.js`, and the spectator window (`bridge.js`) already accepts events over Tauri or SSE. Studio commands are still Tauri IPC names and arguments; a web build needs an HTTP/WebSocket implementation of the same `B` contract.
- **Desktop-only capabilities must stay behind that boundary**: native file dialogs, local paths, OS keychain, installed CLIs, crash reports and updates. A web build replaces them with uploads, server-side storage and account-based credentials rather than reaching the file system directly.
- **Media transport becomes a design concern**: preview frames and uploaded media cross the network in SaaS mode. The existing bounded preview pipeline (cadence, startup reserve, rebuffering) is the basis to extend to remote engines.
- **Testing follows the split**: Rust tests own engine behavior; Node and Chromium tests own UI behavior through the bridge contract. Both remain under the 100% coverage requirement of `docs/QUALITY.md`.
- **Cross-webview behavior must be qualified**: desktop ships on WebView2 (Windows) and WebKitGTK (Linux), and a SaaS would add mainstream browsers. Browser acceptance currently runs Chromium only.

## Alternatives considered

- **Native Rust UI (egui, iced, Slint, gpui)**: best preview performance with no frame transfer, but a full rewrite of the interface and its tests, weaker accessibility and UI-testing tooling, and no path to a browser delivery.
- **Electron instead of Tauri**: same web UI with a bundled Chromium (uniform rendering), at the cost of larger downloads and memory use. Not needed while the system webviews are adequate.
