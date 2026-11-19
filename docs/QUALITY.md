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

The Python job must produce a branch-enabled JSON report before acceptance. `tools/quality/check_python_coverage.py` compares it to tracked Python runtime files under every crate's `src` tree, including nested directories, and all Python files in `tools/quality`. (The former `tools/ci/compare_failures.py` libtest-log checker was removed with the duplicate `ci.yml` workflow; nextest exit status and JUnit reports replace it.) A missing file, missing/invalid counter or nonzero uncovered line/branch fails independently of aggregate rounding. This matters because coverage.py source discovery can omit unimported nested directories without `__init__.py`. Tests must not make that omission invisible.

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

The Linux coverage attempt on `b0e31da` exhausted the hosted runner disk (`No space left on device` in the worker diagnostic log); it produced no acceptable native coverage evidence. Native coverage now uses independent workspace/desktop jobs on each OS, disables incremental compilation and removes only unused preinstalled Android/.NET/GHC/Boost SDK directories on the ephemeral Linux runner. This preserves all test targets, instrumentation and thresholds. It requires a fresh successful run; infrastructure changes alone do not qualify native code.

After the Studio fixture/protocol repairs, the Node 22 / c8 run passed the controller suites and eight additional synthetic bridge cases. Complete UI line coverage was **550/7,412 (7.42%)**; the development bridge contributed 160/250 lines, while playback retained seven uncovered branches. The strict per-file gate correctly failed. Python remained **51 passing tests, 875/875 lines and 190/190 branches**. Studio light/dark, playback UI and proxy badge proofs passed locally against the repaired fixture. These results are scoped evidence, not repository acceptance.

The timeline proof now injects its 720 edits and collects frame work in the browser at the original cadence, retaining all limits and requiring a sample for every edit; its complete local rerun passed. The no-replay recovery variant exposed a real queue-ordering bug: a pre-drop start awaiting paint could remain running forever. A queued gap marker fixes that ordering. The deterministic regression fails against `b0e31da` (the call remains running) and passes with the fix; real SSE replay, refusal and no-replay variants also pass. Original failures are retained, and these segmented local reruns do not replace a complete CI receipt on the final commit.

Final controller-only measurement after the queued-gap fix: **550/7,416 UI lines (7.41%)**. Browser execution remains separate from this c8 report; the 100% JavaScript gate still fails.

## Combined JavaScript execution coverage

The JavaScript job now records controller execution and an additional instrumented Chromium functional suite into the same V8 counter directory. The final c8 report enforces the unchanged per-file 100% lines/functions/branches/statements thresholds and full UI inventory. Either test step failing still fails the job; merging counters does not erase assertion failures.

`tools/quality/browser_coverage.py` starts precise Chromium coverage before page navigation and drains counters before page/context/browser close. It maps only loopback `/ui/*.js` URLs to checkout files, verifies the exact source text and preserves original offsets and counts. A mismatched source or path traversal fails instead of attributing counters to another file. Pages without matching scripts contribute no fabricated report. The pinned Playwright backend retains script sources and counters across navigation (`resetOnNavigation: false`). Its Chromium protocol is used through the Python channel because the Python facade does not expose JavaScript coverage. A SHA-256 source manifest is checked again before merging, so editing source after collection invalidates the report rather than shifting offsets silently.

The additional profiled suite covers the named functional routing/Studio/playback/preview/transition cases and row proofs. Performance-sensitive acceptance still runs separately, without profiling, through the unchanged complete browser inventory. The routing summary script is not used as behavioral acceptance. The collector has tests for response mapping, exact counters, source mismatch, path traversal, empty output, duplicate attachment, close ordering and CLI argument forwarding.

## Per-push and nightly suites

Native tests run under cargo-nextest 0.9.152 (`cargo llvm-cov nextest`), which executes every test in its own process. Inherited tests that change process-wide model, cache and resource environment variables therefore cannot overwrite one another’s configuration, and tests run in parallel instead of the former serial `--test-threads=1` execution. A test that relies on state shared between processes (a fixed cache path, a port) and fails under parallel execution exposes a concurrency defect to fix, not a reason to serialize. The one deliberate exception is kimo's fixed default spectator port range 127.0.0.1:7421-7430, a real system resource: the tests that bind it (`kimo-mcp` `spectator_default` and four `kimo-gui` telemetry/live-feed tests) form the nextest group `spectator-ports`, which runs them one at a time across processes. Their former in-process mutexes could not serialize separate processes. The desktop crate is a separate Cargo workspace, so it has its own `crates/kimo-gui/.config/nextest.toml` with the same two profiles and port group; its heavy set has not been measured yet, so its `ci` profile runs every non-ignored desktop test under the same 30 s per-test limit.

The quality workflow's Rust jobs are the single native test execution on Linux and Windows. The former `ci.yml` workflow re-ran the same Rust suite on every push in an optimized build without coverage; it was removed as a duplicate, since the coverage jobs execute the same tests with debug assertions and overflow checks enabled.

`.config/nextest.toml` splits one suite into two required profiles; no test is removed:

- `ci` runs on every push and pull request: every non-ignored test except the measured heavy set. A test running longer than 30 s is terminated and fails, so a new slow test cannot silently lengthen per-push CI; it must be made faster or deliberately added to the heavy set.
- `nightly` runs on the daily schedule and on manual dispatch: every test, including `#[ignore]` proofs (`--run-ignored all`) and the heavy set, with a 10-minute per-test limit.

The heavy set is every test that took at least 5 s under LLVM instrumentation in the 2026-11-19 parallel run of `59ee16b` (151 of 2,849 workspace tests; they accounted for 6,932 s of 7,696 s of test time, the 65 tests above 30 s alone for 5,915 s). A release requires a passing `nightly` run on the exact release commit in addition to the per-push checks. Per-push coverage is enforced on the `ci` profile alone: a line reached only by a heavy test is uncovered per push and needs a fast, behavioral test. Moving a test between profiles never waives its assertions, missing benchmark inputs, platform-specific subprocess defects or visual proof prerequisites.

Rust coverage CI executes with nextest (which never stops at the first failure under either profile) and `--no-report`, then always attempts a separate LLVM report and exact coverage checks. A failed test still fails its job; it no longer prevents other test targets from executing or hides the counters produced before the failure. Missing reports and incomplete coverage still fail. Desktop jobs build the real MCP engine first; its probe no longer silently returns when the binary is absent.

Native coverage builds retain line-table debug information rather than full debugger type records, with optimization levels and LLVM coverage instrumentation unchanged. This reduces linked test artifact size. Windows selects the C/D volume with most free space for Cargo artifacts, and the real-engine probe path comes from Cargo metadata. Linux workspace jobs install engine build dependencies; GTK/WebKit dependencies remain required in the separate desktop jobs. These changes do not remove test targets or source files.

The complete-source VM tests retain each production module's absolute filename and original text/offsets. The `fix2` and graph-history tests append only export declarations for private helpers and the real router helper bundle, after the entire source, so invalid-input/default cases can be asserted directly. It does not replace function bodies, slice source, alter counters or exclude unreachable-looking code. Browser source verification remains exact for browser-collected profiles.

Browser profiles use local `file://` URLs (with URI escaping), as Node's V8 profiles do. c8 10.1.3 records executed files in its `--all` inventory only after converting file URLs; plain absolute path URLs otherwise create an extra synthetic `(empty-report)` function alongside real browser execution. This mapping correction preserves all source hashes, offsets, counters, files and thresholds; the collector test asserts the complete emitted profile and stale-source rejection.
