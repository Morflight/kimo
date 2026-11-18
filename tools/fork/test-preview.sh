#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
for name in playback_delivery playback_sync playback_buffer playback_rebuffer playback_prepare prepare_ui; do
  node "crates/kimo-gui/tests/${name}_test.mjs"
done
node --check crates/kimo-gui/ui/playback.js
node --check crates/kimo-gui/ui/studio.js
git diff --check
