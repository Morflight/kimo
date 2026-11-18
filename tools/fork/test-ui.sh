#!/usr/bin/env bash
# Complete-source controller tests; browser acceptance is a separate suite.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash tools/fork/test-preview.sh
node --experimental-vm-modules crates/kimo-gui/tests/bridge_test.mjs
node crates/kimo-gui/tests/playback_lifecycle_test.mjs
