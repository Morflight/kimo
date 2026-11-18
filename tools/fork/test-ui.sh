#!/usr/bin/env bash
# Complete-source controller tests; browser acceptance is a separate suite.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash tools/fork/test-preview.sh
node --experimental-vm-modules crates/kimo-gui/tests/bridge_test.mjs
node crates/kimo-gui/tests/playback_lifecycle_test.mjs
node --experimental-vm-modules crates/kimo-gui/tests/mock_bridge_test.mjs
node --experimental-vm-modules crates/kimo-gui/tests/modes_test.mjs
node --experimental-vm-modules crates/kimo-gui/tests/read_format_test.mjs
node --experimental-vm-modules crates/kimo-gui/tests/project_ops_test.mjs
