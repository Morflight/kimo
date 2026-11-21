# ADR 0001: JavaScript web interface, Rust engine, native Rust desktop app

- Status: accepted
- Date: 2026-11-19
- Decider: repository owner

## Context

Kimo is a video editor. Its current desktop application (`crates/kimo-gui`) is a Tauri 2 app: a Rust process plus an interface rendered in the system webview from `crates/kimo-gui/ui` (`"frontendDist": "ui"` in `tauri.conf.json`). All visible UI is HTML/CSS/JavaScript (about 7,000 lines: Studio, spectator, timeline, panels, playback). Decoding, rendering, color, analysis and export run in Rust (`kimo-core`, served by the `kimo-mcp` engine).

The question was where the interface should live, given a possible SaaS offering and the need to use editors' own hardware for performance. A webview UI is cheap to build and test for dense layouts and can be served in a browser; a native Rust UI avoids transferring preview frames from the engine to a webview.

## Decision

1. **The JavaScript interface is the web UI**, so the product can be delivered in a browser if Kimo becomes a SaaS product.
2. **All heavy lifting stays in Rust**: decoding, rendering, compositing, color, audio, analysis and export. Interfaces display results and send user intent; they do not process media.
3. **Delivery follows the usual pattern of serious web services**: the web UI is the default entry point of a SaaS offering, and editors with capable computers install a desktop application that uses their own hardware for better performance.
4. **The desktop application will be a native Rust application.** Its development is deliberately deferred (see Priority).
5. **When rendering becomes too expensive for the user's machine** (web or desktop), the product can provide compute on remote instances running the same Rust engine.

## Priority

The native Rust desktop app is not a current priority. The priority is that the software delivers **reviewable edits**: edits an editor can inspect, compare and accept or undo, through the existing interface. Until the native desktop app is built, the current Tauri app remains the shipped desktop interface and stays fully under the quality contract (`docs/QUALITY.md`).

## Consequences

- **The engine boundary is the product's architecture.** Every interface (web UI, current Tauri app, future native Rust app) must reach the engine through one transport-neutral contract, so it can talk to a local engine (desktop) or a remote one (SaaS, remote render). Today this boundary partly exists: `studio.js` goes through a bridge object `B` (`invoke`, `listen`) that browser tests replace with `ui/dev/mock-bridge.js`, and the spectator window (`bridge.js`) already accepts events over Tauri or SSE. Studio commands are still Tauri IPC names and arguments; a web build needs an HTTP/WebSocket implementation of the same `B` contract, and the native desktop app will use the same engine contract directly.
- **Two interfaces will eventually exist** (JavaScript web, native Rust desktop). Product behavior that both need, such as edit review, history and undo, belongs in the engine contract rather than in either UI, so it is implemented and tested once.
- **Desktop-only capabilities stay behind the boundary**: native file dialogs, local paths, OS keychain, installed CLIs, crash reports and updates. A web build replaces them with uploads, server-side storage and account-based credentials rather than reaching the file system directly.
- **Media transport becomes a design concern**: preview frames and uploaded media cross the network in SaaS mode. The existing bounded preview pipeline (cadence, startup reserve, rebuffering) is the basis to extend to remote engines.
- **Testing follows the split**: Rust tests own engine behavior; Node and Chromium tests own web UI behavior through the bridge contract. All first-party code remains under the 100% coverage requirement of `docs/QUALITY.md`.
- **Browser behavior must be qualified for the web UI**: browser acceptance currently runs Chromium only; a SaaS adds mainstream browsers, and the current desktop app ships on WebView2 (Windows) and WebKitGTK (Linux).

## Alternatives considered

- **Native Rust UI as the only interface (egui, iced, Slint, gpui)**: rejected, because it gives no path to browser delivery.
- **Shared JavaScript interface on desktop as well (one UI codebase)**: rejected for the long term in favor of a native Rust desktop app that makes full use of local hardware without webview frame transfer; it remains the interim desktop interface.
- **Electron instead of Tauri for the interim desktop app**: same web UI with a bundled Chromium (uniform rendering), at the cost of larger downloads and memory use. Not needed while the system webviews are adequate.
