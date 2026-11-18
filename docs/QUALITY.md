# Quality contract

The acceptance target is **100% line and function coverage of first-party runtime code**, plus 100% branch coverage wherever measured. All required tests must run and pass. There is no legacy-failure waiver. This is a requirement, not a claim that the inherited code currently meets it.

## Current status

The fork inherited 431 Rust source files and 13 non-vendored UI JavaScript files at the initial audit. Its feature reference is substantial but historical, and its native gate manifest was stale. The six preview/controller regression suites passed; that does not establish complete feature coverage, native coverage, browser coverage or real audiovisual correctness. A repository-wide measured 100% result has not been obtained. Until it is obtained, this baseline is **not quality-qualified for release** under this contract.

## Change procedure

1. Identify affected entries in `docs/features/catalog.json`; read the linked feature contracts.
2. Change implementation, contracts and behavioral tests together. Cover success, invalid inputs, boundaries, cancellation and lifecycle errors as appropriate.
3. Run documentation coherence checks against the pre-change commit (or an optional PR base), unit/integration tests, and native/browser coverage. Preserve reports for the exact commit.
4. Inspect coverage gaps; add meaningful tests. Never lower thresholds, silently skip tests, regenerate goldens or accept failures as a new baseline.
5. Commit and push directly to `main` in the solo workflow; branches and PRs are optional. Every push runs CI, including documentation-only changes. All required tests must pass and coverage must reach 100% before claiming quality acceptance or releasing. A missing runner/tool/dependency is a qualification blocker, not a waiver.

The catalog's `legacy-unverified` state explicitly records inherited documentation debt. It is not a certification. Each affected legacy area must gain a focused contract and linked behavioral tests before new behavior can be accepted there. Mechanical linkage checks cannot prove prose semantics; maintainer inspection is still required.

## Required evidence

- Documentation: existing links, classified first-party source files, implementation changes accompanied by contract and test changes.
- Tests: successful exit, nonzero executed tests, no unexpected skips/ignored tests, exact target/platform/features, complete logs.
- Coverage: reports for all first-party runtime files, 100% executable lines/functions, and branch coverage when available. Files missing from reports are failures. Rust conditional code requires Windows and Linux evidence; neither target substitutes for the other.
- Product acceptance: synthetic end-to-end editing/playback/export cases; native presentation and physical sound are separately reported. Controller simulations do not certify real audio/video playback.

## Existing commands and limitations

`bash tools/fork/test-preview.sh` runs the six focused JavaScript suites.
`python3 tools/quality/check_contracts.py --base <commit>` checks linkage and change coherence.
`bash tests/gate_fast.sh` and `bash tests/gate_full.sh` are inherited native/proof runners. Their manifest now inventories all current native suites, and process-name cleanup was removed. They still need a complete successful run before being treated as qualified acceptance gates.

The GitHub quality workflow enforces contract checks and attempts strict Rust/JavaScript coverage. Failures are expected until inherited coverage debt is resolved; they must remain visible. Full feature-matrix and browser acceptance qualification is still outstanding and must not be inferred from a green focused preview job.

Coverage jobs use pinned collectors (cargo-llvm-cov 0.9.1, c8 10.1.3, coverage.py 7.6.12). JavaScript includes all non-vendored UI files, even if untouched by tests. Rust compares LLVM reports against the tracked runtime source inventory and rejects absent files. Python includes the core runtime sidecar and quality/CI tooling. The present collectors do not certify inline JavaScript embedded in Rust/HTML, optional native feature combinations, C/C++ runtime bridges or build scripts; these remain explicit qualification blockers, not covered exclusions. Branch counters unavailable from a stable Rust collector must be reported as unavailable, never 100%.

## Initial measured JavaScript evidence

On 2026-11-18, all six controller suites passed under Node 22 with c8 10.1.3. The UI-only line coverage in that run was **3.42% (252/7,378 lines, including the development mock)**, with **75.90%** for playback.js. Twelve other UI files had no mapped execution from these controller suites. Sliced editor-controller tests are not attributed to the complete studio.js file, because fragment offsets would give misleading coverage. This is coverage from this specific suite, not a statement that no other tests exist. It fails the required 100% threshold.

## Solo workflow authorization

On 2026-11-18 the owner explicitly authorized moving the initial documentation and controls onto main with their known coverage gaps. Mandatory PR/review restrictions are removed. Required checks remain configured, with owner/admin bypass enabled to permit direct development pushes. CI failures remain visible and the 100% thresholds and strict acceptance job are unchanged. This integration is not evidence that inherited failures or coverage debt have been resolved.

## Python evidence and inventory

The Python job must produce a branch-enabled JSON report before acceptance. `tools/quality/check_python_coverage.py` compares it to tracked Python runtime files under every crate's `src` tree, including nested directories, and all Python files in `tools/quality` and `tools/ci`. A missing file, missing/invalid counter or nonzero uncovered line/branch fails independently of aggregate rounding. This matters because coverage.py source discovery can omit unimported nested directories without `__init__.py`. Tests must not make that omission invisible.

The helper protocol suites run under unittest discovery alongside quality-control tests. Code clip browser/Pillow and OpenColorIO processor doubles establish protocol handling only; real browser pixels, real OCIO transforms and platform behavior remain qualification requirements. Coverage.py measures lines and branches, not an independent function counter. Inlined JavaScript inside the sidecar is still subject to the existing inline-JavaScript blocker.

On 2026-11-18 the expanded local Python suite passed 43 tests under Python 3.10 / coverage.py 7.6.12: **739/739 executable lines and 174/174 measured branches** across the two runtime helpers and quality/CI Python tooling, including tests. The explicit inventory check, feature coherence check and 448-entry native manifest check passed. This scoped result does not certify JavaScript, Rust, embedded JavaScript, real browser/OCIO behavior or the wider repository's Python proof scripts.

The expanded Node 22 / c8 10.1.3 controller run also passed all six preview suites and eight event-bridge cases. `bridge.js` measured **58/58 lines, 7/7 functions and 35/35 branches**. Full-UI line coverage in that run was **310/7,378 (4.20%)**; strict per-file acceptance correctly failed. Browser-script results are separate and are not represented by this controller-only coverage number. Use `bash tools/fork/test-ui.sh` for the expanded controller suite.

## Required browser proofs

The quality workflow now includes `browser-acceptance`, and the aggregate acceptance job requires it. It installs Playwright 1.58.0, its matching Chromium headless shell and Pillow 12.1.0 on Linux. Run the same suite with:

```sh
python3 tools/quality/run_browser_tests.py --output /path/to/browser-results
```

The runner discovers every `crates/kimo-gui/tests/*_test.py` plus `idle_proof.py`, then runs dark-theme studio, refused-reconnect and no-replay variants. Each script has a 300-second deadline. It writes individual logs and an incrementally updated `results.json`; launch errors, timeouts, nonzero exits and empty logs fail the run, while remaining scripts still execute. A timeout terminates only that script's newly created process group. Screenshot capture is disabled for the routine gate. The runner is intentionally POSIX-only; Windows desktop acceptance remains separate.

`coverage_test.py` is a tool-family routing report, not a code-coverage measurement or behavioral pass. Its receipt is explicitly labelled `report`. The other scripts own their assertions and exit status. Browser proofs exercise synthetic local feeds and the development bridge, not the user's engine, projects or physical audio. Their coverage is not yet merged into c8: passing browser tests cannot be described as 100% JavaScript coverage. The browser qualification blocker remains until the integrated gate and required scope are actually validated.

## Native collector and fixture repairs

The initial Linux run on `7dfb17b` failed to compile `color_cache` because two dense-frame fixtures omitted the newly required `Frames.step`. Both now explicitly use stride 1; their pixel/window assertions are retained. GUI library execution in that coverage job passed 81 tests, but collector 0.6.16 produced no object filenames and therefore no report. That is not successful coverage.

The collector is now pinned to 0.9.1, which includes the new Cargo build-directory layout support and Windows path matching fixes documented in the [upstream releases](https://github.com/taiki-e/cargo-llvm-cov/releases/tag/v0.9.1). Matching the observed `debug/build/.../out` test executable layout to that issue is a diagnosis; a new successful report is still required to validate the repair. Both native workflows now install the pinned Playwright/Pillow runtime and Chromium required by the existing real code-clip integration tests. App tests run even after engine test failure so one failure cannot suppress desktop execution. Optional native configurations and tests that return early when oracle/config inputs are absent remain unqualified.

The next local run passed **51 Python tests**, with **875/875 executable lines and 190/190 measured branches** including the browser runner and its failure-path tests. The completed Node 22 controller run passed all prior suites, eight bridge cases and seventeen lifecycle cases: playback.js reached **332/332 lines, 45/45 functions and 316/323 branches**; full UI lines were **390/7,378 (5.28%)**. The strict JavaScript gate still fails. No branch counter or runtime file was excluded to obtain these results.

The first full local browser pass exposed two obsolete/test-harness assumptions: cross-process setup let the first of three supposedly overlapping 700 ms flashes expire, and a blanket Rust-source text ban rejected the intentional active viewer's render calls. Both received behavioral repairs and focused contracts. Targeted reruns passed 10/10 flash assertions and 98 revision assertions, respectively. The original failed logs remain evidence; they are not accepted as a baseline.
