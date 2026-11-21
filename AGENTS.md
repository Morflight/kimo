# Kimo maintenance contract

## Authority and scope
- Work on Morflight/kimo; `main` is the default integration branch. Original upstream is naman-sharma12345/kimo. Preserve its history, MIT license and notices.
- Read docs/TESTING_STRATEGY.md, docs/FORK.md, docs/QUALITY.md and docs/features/README.md before changing behavior or tests. docs/TESTING_STRATEGY.md is normative and overrides any older instruction about branches or test suites.

## Lifecycle and test suites (docs/TESTING_STRATEGY.md)
- Every feature, fix or improvement gets its own branch. NEVER develop or commit work directly on `main`; `main` changes only by merging a pull request.
- Every push runs the fast suite (nextest profile `fast`). Pull requests to `main` and pushes to `main` run the heavy suite (profile `heavy`: every test, including `#[ignore]` proofs). Merge only when every required check passes, heavy suite included.
- Every feature behavior MUST have a fast test; fast tests that process media MUST use tiny synthetic media and assert that behavior is unchanged. 100% coverage MUST be reached by the fast suite alone.
- Heavy tests exist only for accuracy, performance, visual dumps or full pipelines at realistic scale. Each MUST be registered in `.config/heavy-tests.json` with its category and the fast test covering the same behavior, and `python3 tools/quality/check_test_strategy.py` MUST pass.
- NEVER move a test to the heavy register, mark it `#[ignore]`, lengthen the 30 s fast limit, weaken an assertion or raise `LEGACY_UNTRIAGED_MAX` to make a change pass.
- Do not change branch strategy, acceptance thresholds, coverage scope, golden outputs or failure baselines to make a change pass.
- Keep real projects/history, media, credentials, caches and binaries outside Git. Never stop a process by executable name; terminate only children created by the current test.

## Feature, documentation and test coherence
- Every behavior change must update its feature contract and automated tests in the same change. Contracts describe inputs/defaults, visible behavior, errors, limits and acceptance cases.
- Register production paths, documentation and tests in docs/features/catalog.json. New production files must be classified; broad claims without tests are not evidence.
- Update the feature inventory for additions/removals. Link implementation and tests; never mark a planned, partially tested or unsupported behavior as verified.
- Run `python3 tools/quality/check_contracts.py --base <base-commit>`, `python3 tools/quality/check_test_strategy.py` and all applicable tests for every change. CI runs on every push and every pull request; a docs-only change still runs CI.

## Acceptance: no silent waivers
- All required tests must execute and pass. A skipped test, missing dependency/report, empty test run, timeout, crash or unmeasured platform is not a pass.
- Required code coverage is 100% of executable lines and functions in first-party Rust and JavaScript production code, on every supported target. Branch coverage must also reach 100% where the collector supports it. Never round a result up to 100%.
- Untested files count against coverage. Do not shrink the denominator, exclude difficult code, add coverage-ignore annotations or replace behavioral assertions with execution-only tests.
- Third-party vendored code and generated artifacts are not first-party implementation; exclusions must be explicit in the checked-in policy. Tests/tools require their own tests; do not describe runtime coverage as coverage of every repository language.
- Do not release or claim quality acceptance while any required check is absent, pending or failed. The owner explicitly authorized integrating the initial quality controls into main despite documented inherited gaps; this does not waive tests or lower coverage thresholds. Never turn inherited failures into an automatically accepted baseline.
- Report exact commands, commit, results, coverage scope and unverified behavior. 100% code coverage is necessary, not proof of correctness or physical audio/visual quality.

## Builds and operations
- Use committed lockfiles and the pinned build toolchain. Keep GUI and engine identity consistent and use the fork configuration.
- Check available storage before substantial builds/media work on the editing workstation. Keep caches external to source and preserve SSD folder links.
- Test synthetic data in isolated directories/profiles. Never restart the user's live editor or overwrite a project to run tests.
