import {fileURLToPath} from 'node:url';
// Run: node crates/kimo-gui/tests/playback_delivery_test.mjs
// Set KIMO_PLAYBACK_SOURCE to test the unpatched player against the same regressions.
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const src = readFileSync(process.env.KIMO_PLAYBACK_SOURCE || new URL('../ui/playback.js', import.meta.url), 'utf8');
function harness(fetcher = async () => ({ ok: true, blob: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }), decode = async () => ({ close() {} })) {
  let now = 0, next = 0;
  const callbacks = new Map(), errors = [], states = [];
  const ctx = { window: { createImageBitmap: decode }, createImageBitmap: decode, fetch: fetcher,
    performance: { now: () => now }, requestAnimationFrame: f => { callbacks.set(++next, f); return next; },
    cancelAnimationFrame: id => callbacks.delete(id), clearTimeout() {}, setTimeout() {} };
  vm.createContext(ctx); vm.runInContext(src, ctx, {filename: fileURLToPath(new URL('../ui/playback.js', import.meta.url))});
  const p = new ctx.window.KimoPlayer({ fps: () => 30, duration: () => 10, scale: () => .5,
    onError: e => errors.push(e), onState: s => states.push(s), invoke: async () => ({}) });
  p.gen = 1; p.info = { audio_on: false }; p.playing = true;
  return { p, errors, states, advance(ms) { now += ms; const jobs = [...callbacks.values()]; callbacks.clear(); jobs.forEach(f => f()); } };
}
let failures = 0;
async function test(name, f) { try { await f(); console.log('ok', name); } catch (e) { failures++; console.error('FAIL', name, e.message); } }
await test('rejected fetch ends buffering and reports the delivery failure', async () => {
  const h = harness(async () => { throw new TypeError('Failed to fetch'); });
  await h.p.fetchFrame({ index: 0, url: 'http://kimo.localhost/00.bmp' });
  h.p.rebuffer(); h.advance(15001);
  assert.equal(h.p.playing, false); assert.equal(h.p.buffering, false); assert.match(h.errors[0], /Failed to fetch/);
});
await test('HTTP failures are reported rather than decoded', async () => {
  const h = harness(async () => ({ ok: false, status: 404 }));
  await h.p.fetchFrame({ index: 0, url: 'gone' }); h.p.rebuffer(); h.advance(15001);
  assert.match(h.errors[0], /HTTP 404/);
});
await test('decode failures are surfaced after bounded buffering', async () => {
  const h = harness(undefined, async () => { throw new Error('bad bitmap'); });
  await h.p.fetchFrame({ index: 0, url: 'bad' }); h.p.rebuffer(); h.advance(15001);
  assert.match(h.errors[0], /bad bitmap/);
});
await test('missing engine frames also time out without inventing a network cause', () => {
  const h = harness(); h.p.rebuffer(); h.advance(15001);
  assert.equal(h.p.playing, false); assert.match(h.errors[0], /timed out/);
});
await test('a transient missing frame can recover before the timeout', async () => {
  let failed = false;
  const h = harness(async () => { if (!failed) { failed = true; throw new Error('evicted'); } return { ok: true, blob: async () => ({}) }; });
  await h.p.fetchFrame({ index: 0, url: '0' }); h.p.rebuffer(); h.advance(100);
  for (let i = 0; i < 32; i++) await h.p.fetchFrame({ index: i, time: i / 30, url: String(i) });
  h.advance(100); assert.equal(h.p.buffering, false); assert.ok(h.p.clock); assert.equal(h.errors.length, 0);
});
await test('pause cancels the pending timeout', () => {
  const h = harness(); h.p.rebuffer(); h.p.pause(); h.advance(15001); assert.equal(h.errors.length, 0);
});
await test('failures from a replaced generation do not contaminate the new run', async () => {
  let reject; const h = harness(() => new Promise((_, no) => { reject = no; }));
  const pending = h.p.fetchFrame({ index: 0, url: 'old' }); h.p.gen = 2; reject(new Error('old generation')); await pending;
  assert.ok(!h.p.frameError);
});
await test('audio failures are surfaced once per generation', async () => {
  const h = harness(async () => { throw new Error('audio blocked'); }); h.p.ctx = {};
  await h.p.fetchChunk({ name: '00.wav', url: 'audio' }); await h.p.fetchChunk({ name: '00.wav', url: 'audio' });
  assert.equal(h.errors.length, 1); assert.match(h.errors[0], /audio blocked/);
});
process.exitCode = failures ? 1 : 0;
