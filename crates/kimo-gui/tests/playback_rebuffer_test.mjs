import {fileURLToPath} from 'node:url';
// Run: node crates/kimo-gui/tests/playback_rebuffer_test.mjs
// A 60 fps player must not rewind behind frames the engine has already evicted on an underrun.
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const src = readFileSync(new URL('../ui/playback.js', import.meta.url), 'utf8');
let ms = 0, id = 0, lastPublishedFrame = 0;
const jobs = new Map(), states = [];
const c = { window: {}, performance: { now: () => ms },
  requestAnimationFrame: f => { jobs.set(++id, f); return id; }, cancelAnimationFrame: n => jobs.delete(n),
  clearTimeout() {}, setTimeout() {} };
vm.createContext(c); vm.runInContext(src, c, {filename: fileURLToPath(new URL('../ui/playback.js', import.meta.url))});
const p = new c.window.KimoPlayer({ fps: () => 60, duration: () => 10,
  canvas: { width: 1, height: 1, getContext: () => ({ drawImage() {} }) },
  onTime() {}, onState: s => states.push(s), scale: () => .5 });
p.info = { audio_on: false }; p.playing = true; p.run = { start: 0, dir: 1, stride: 1 };
for (let i = 0; i < 8; i++) p.frames.set(i, { bmp: { width: 1, height: 1 }, time: i / 60 });
p.go();
for (ms = 10; ms < 1000 && !p.buffering; ms += 10) {
  const callbacks = [...jobs.values()]; jobs.clear(); callbacks.forEach(f => f());
  if (ms % 120 === 0) lastPublishedFrame = Math.floor(p.t * 60 + 1e-6);
}
assert.equal(p.buffering, true, 'the missing frames trigger rebuffering');
const resumeFrame = Math.floor(p.t * 60 + 1e-6);
console.log(JSON.stringify({ lastPublishedFrame, resumeFrame, engineBehind: 8 }));
assert.ok(resumeFrame >= lastPublishedFrame, 'an underrun must not replay audio already heard');
assert.ok(resumeFrame + 8 >= lastPublishedFrame, 'rebuffering must stay inside the engine retention window');
for (let i = resumeFrame; i < resumeFrame + 32; i++) p.frames.set(i, { bmp: { width: 1, height: 1 }, time: i / 60 });
ms += 10;
const callbacks = [...jobs.values()]; jobs.clear(); callbacks.forEach(f => f());
assert.equal(p.buffering, false, 'playback resumes after the required frames arrive');
assert.ok(p.t * 60 >= lastPublishedFrame, 'the resumed clock remains monotonic');
console.log('ok underrun preserves the clock and recovers without an implicit rewind');
