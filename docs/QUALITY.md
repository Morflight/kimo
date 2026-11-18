# Quality contract

The acceptance target is **100% line and function coverage of first-party runtime code**, plus 100% branch coverage wherever measured. All required tests must run and pass. There is no legacy-failure waiver. This is a requirement, not a claim that the inherited code currently meets it.

## Current status

The fork inherited 431 Rust source files and 13 non-vendored UI JavaScript files at the initial audit. Its feature reference is substantial but historical, and its native gate manifest was stale. The six preview/controller regression suites passed; that does not establish complete feature coverage, native coverage, browser coverage or real audiovisual correctness. A repository-wide measured 100% result has not been obtained. Until it is obtained, this baseline is **not quality-qualified for merge/release** under this contract.

## Change procedure

1. Identify affected entries in `docs/features/catalog.json`; read the linked feature contracts.
2. Change implementation, contracts and behavioral tests together. Cover success, invalid inputs, boundaries, cancellation and lifecycle errors as appropriate.
3. Run documentation coherence checks against the PR base, unit/integration tests, and native/browser coverage. Preserve reports for the exact commit.
4. Inspect coverage gaps; add meaningful tests. Never lower thresholds, silently skip tests, regenerate goldens or accept failures as a new baseline.
5. Merge only after required CI checks pass. Documentation-only changes also require checks. A missing runner/tool/dependency is a blocker, not a waiver.

The catalog's `legacy-unverified` state explicitly records inherited documentation debt. It is not a certification. Each affected legacy area must gain a focused contract and linked behavioral tests before new behavior can be accepted there. Mechanical linkage checks cannot prove prose semantics; reviewer inspection is still required.

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

Coverage jobs use pinned collectors (cargo-llvm-cov 0.6.16, c8 10.1.3, coverage.py 7.6.12). JavaScript includes all non-vendored UI files, even if untouched by tests. Rust compares LLVM reports against the tracked runtime source inventory and rejects absent files. Python includes the core runtime sidecar and quality/CI tooling. The present collectors do not certify inline JavaScript embedded in Rust/HTML, optional native feature combinations, C/C++ runtime bridges or build scripts; these remain explicit qualification blockers, not covered exclusions. Branch counters unavailable from a stable Rust collector must be reported as unavailable, never 100%.

## Initial measured JavaScript evidence

On 2026-11-18, all six controller suites passed under Node 22 with c8 10.1.3. The UI-only line coverage in that run was **3.42% (252/7,378 lines, including the development mock)**, with **75.90%** for playback.js. Twelve other UI files had no mapped execution from these controller suites. Sliced editor-controller tests are not attributed to the complete studio.js file, because fragment offsets would give misleading coverage. This is coverage from this specific suite, not a statement that no other tests exist. It fails the required 100% threshold.
