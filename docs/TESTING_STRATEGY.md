# Testing strategy and development lifecycle

This document is normative. It applies to every contributor, human or AI agent. "MUST" and "NEVER" are requirements, not advice. `tools/quality/check_test_strategy.py` enforces the mechanical parts in CI; the rest is enforced by review. Decided by the repository owner on 2026-11-19.

## 1. Two suites

| Suite | nextest profile | Runs on | Contents | Purpose |
|---|---|---|---|---|
| **Fast** | `fast` | every push, on every branch | every test except the registered heavy tests and `#[ignore]` proofs | fast feedback while iterating; full behavioral coverage |
| **Heavy** | `heavy` | pull requests to `main`, pushes to `main`, manual dispatch | every test, including heavy tests and `#[ignore]` proofs (`--run-ignored all`) | accuracy and performance on realistic media; last safeguard before `main` changes |

Both suites run under LLVM coverage on Linux and Windows (`.github/workflows/quality.yml`). JavaScript (Node + Chromium) and Python suites run on every push, as before.

## 2. Development lifecycle

1. **Every feature, fix or improvement gets its own branch.** Iterate there; each push runs the fast suite.
2. **NEVER develop directly on `main`.** `main` only changes by merging a branch through a pull request.
3. **Open the pull request when the branch is ready to merge.** The pull request runs the heavy suite. Iterating after that is possible but pays the heavy cost on each push, so prefer finishing on the branch first.
4. **Merge only when every required check passes, including the heavy suite.** A failing heavy test blocks the merge: fix the cause, never the threshold.
5. A push that lands on `main` runs the heavy suite again on the merged result.

## 3. Rules for tests

### Where tests live

- **Unit tests** go in a `#[cfg(test)] mod tests` at the bottom of the source file they test.
- **Integration tests** of a crate are one test binary, `all`: each `crates/<crate>/tests/all/<name>.rs` is a module declared in `tests/all/main.rs`, and shared fixtures live in `tests/all/common/`. Add a new file there and declare it with `mod <name>;`; do not add top-level `tests/*.rs` files (the crates set `autotests = false`). One binary links once instead of once per file, which keeps local and CI builds fast.
- nextest names an integration test `<crate>::all` / `<name>::<test>`; the register and filters use those ids (for example `binary_id(kimo-core::all) & test(=auto_edit::some_test)`). nextest still runs every test in its own process, so tests that set environment variables stay isolated.
- Exceptions, separate binaries: `kimo-core` `frame_live_peak` (its `#[global_allocator]` would apply to every test of a shared binary) and `libav_vfr_conform` (needs the `libav` feature).

### Fast tests (the default)

- **Every feature behavior MUST be tested by a fast test.** A heavy test is never the only test of a behavior.
- **Fast tests that process media MUST use tiny media**: the smallest synthetic input that exercises the behavior, typically generated in the test (FFmpeg `lavfi` sources such as `color`, `nullsrc` + `geq`, `sine`, or in-code buffers). Typical sizes are a few dozen pixels per side, a few frames or seconds, and expected values computed by hand. See `crates/kimo-core/tests/all/time_remap.rs` (32×16, 10 fps, exact pixel expectations).
- **A fast test asserts that behavior is unchanged**: exact or tolerance-bounded results that fail when the behavior regresses. Execution-only tests are not tests.
- A fast test runs in seconds. The `fast` profile kills any test after 30 s, which fails the suite. Under instrumentation, a fast test should stay well below 5 s.
- **100% coverage is reached by the fast suite alone.** A line reached only by a heavy test is uncovered: add a fast tiny-media test for it. NEVER satisfy coverage through the heavy suite.

### Heavy tests (the exception)

A test is heavy only when its purpose needs realistic scale:

| Category | Meaning | Example |
|---|---|---|
| `accuracy` | measures quality or precision on realistic or large media | stabilization residual on a 1080p shaky plate |
| `performance` | measures time, throughput or memory at realistic scale | 1080p three-track playback at 25 fps |
| `visual-dump` | produces frames for human inspection | 1080p contact sheets of a renderer |
| `pipeline-at-scale` | runs a complete pipeline on realistic media end to end | full auto-edit of a real-length clip |

Every heavy test MUST be registered in `.config/heavy-tests.json` with its `category` and its `fast_test`: the fast tiny-media test that covers the same behavior. The heavy test only adds evidence at scale.

## 4. Adding or changing a heavy test

1. Write the fast tiny-media test first, covering the behavior.
2. Add the heavy test to `.config/heavy-tests.json` (sorted by binary, then test), with a category and `fast_test` naming the fast test as `binary::test` (an integration test is `<crate>::all::<name>::<test>`).
3. Run `python3 tools/quality/check_test_strategy.py`; copy the `default-filter` it prints into `[profile.fast]` of `.config/nextest.toml`.
4. NEVER mark a test `#[ignore]` to keep it out of the fast suite; use the register. Existing `#[ignore]` proofs remain heavy-only legacy.

## 5. Legacy heavy tests

On 2026-11-19, 151 tests took at least 5 s under instrumentation and were moved out of the fast suite as `untriaged` (`LEGACY_UNTRIAGED_MAX` in the checker). Many of them test ordinary feature behavior on unnecessarily large media. Each one must be triaged:

- **feature behavior on large media**: rewrite it on tiny media and remove it from the register (it becomes a fast test), or add a fast tiny-media test and keep it heavy with a category;
- **genuine accuracy, performance, dump or full-scale pipeline**: keep it heavy, set its category and `fast_test`.

Lower `LEGACY_UNTRIAGED_MAX` to the remaining count in the same change. The checker fails if untriaged entries exceed it; NEVER raise it.

## 6. What the checker enforces

`tools/quality/check_test_strategy.py` runs in the `feature-contracts` CI job and fails when:

- the `fast` default-filter is not exactly the register (a test excluded without registration, or a registered test still running in the fast suite);
- the `heavy` profile is missing or filters tests;
- a triaged heavy test has no known category, no `fast_test`, a `fast_test` that is not defined under `crates/`, or a `fast_test` that is itself heavy;
- the register is unsorted, has duplicates or malformed entries;
- untriaged entries exceed the legacy baseline.

It cannot judge whether a fast test really covers the same behavior or whether media is "tiny": reviewers and agents MUST apply sections 3 and 4.

## 7. Forbidden shortcuts

NEVER, to make a change pass:

- move a test to the heavy register, mark it `#[ignore]`, lengthen the 30 s fast limit or weaken an assertion;
- rely on the heavy suite for coverage;
- raise `LEGACY_UNTRIAGED_MAX`;
- merge into `main` with a failing or missing heavy suite, or develop directly on `main`;
- change coverage thresholds, coverage scope, golden outputs or failure baselines (see `docs/QUALITY.md`).
