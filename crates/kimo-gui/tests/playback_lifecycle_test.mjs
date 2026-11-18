import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const filename = fileURLToPath(new URL('../ui/playback.js', import.meta.url));
const source = readFileSync(filename, 'utf8');
const settle = () => new Promise(resolve => setImmediate(resolve));
function deferred() { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
function harness({ invoke, fetcher, bitmap = true, audio, global = false } = {}) {
  let now = 1000, id = 0;
  const calls = [], errors = [], states = [], draws = [], bitmaps = [], timers = new Map(), raf = new Map();
  const makeBitmap = () => { const b = { width: 2, height: 1, closed: 0, close() { this.closed++; } }; bitmaps.push(b); return b; };
  const root = { ...(bitmap ? { createImageBitmap: async () => makeBitmap() } : {}), ...(audio || {}) };
  const ctx = vm.createContext({ ...(global ? root : { window: root }), performance: { now: () => now },
    createImageBitmap: async () => makeBitmap(), Image: class { constructor() { Object.assign(this, makeBitmap()); } set src(_) { queueMicrotask(() => this.onload()); } },
    URL: { createObjectURL: () => 'blob:fixture' },
    fetch: fetcher || (async () => ({ ok: true, blob: async () => ({}), arrayBuffer: async () => new ArrayBuffer(4) })),
    requestAnimationFrame: f => { raf.set(++id, f); return id; }, cancelAnimationFrame: n => raf.delete(n),
    setTimeout: (f, ms) => { timers.set(++id, { f, ms }); return id; }, clearTimeout: n => timers.delete(n) });
  vm.runInContext(source, ctx, { filename });
  const o = { fps: () => 30, duration: () => 10, scale: () => 0.5,
    canvas: { width: 0, height: 0, getContext: () => ({ drawImage: (...a) => draws.push(a) }) },
    invoke: async (command, args) => { calls.push({ command, args }); return invoke ? invoke(command, args) : { gen: 1, audio_on: false }; },
    onError: e => errors.push(e), onState: (...s) => states.push(s), onTime() {} };
  const player = new (global ? ctx.KimoPlayer : root.KimoPlayer)(o);
  return { av: global ? ctx.KimoAV : root.KimoAV, p: player, o, calls, errors, states, draws, bitmaps, timers, raf, makeBitmap,
    advance(ms) { now += ms; const jobs = [...raf.values()]; raf.clear(); jobs.forEach(f => f()); } };
}
let count = 0;
async function test(name, fn) { await fn(); count++; console.log('ok', name); }
await test('unsupported preview is remembered; other errors remain retryable', async () => {
  const unavailable = harness({ invoke: async () => { throw Error('unknown command preview_start'); } });
  assert.equal(await unavailable.p.ensure(0), false); assert.equal(await unavailable.p.play(0), false);
  assert.equal(unavailable.calls.length, 1); assert.equal(unavailable.errors.length, 0);
  const broken = harness({ invoke: async () => { throw Error('disk failure'); } });
  assert.equal(await broken.p.ensure(0), false); assert.match(broken.errors[0], /disk failure/);
  const h = harness({ global: true }); assert.equal(await h.p.ensure(0), true); assert.equal(await h.p.ensure(1), true);
  assert.equal(h.calls.length, 1); h.o.fps = () => 0; h.o.duration = () => 0;
  assert.equal(h.p.fps, 30); assert.equal(h.p.end, 0);
});
await test('stop closes frames and clears audio even if engine stop rejects', async () => {
  const h = harness({ invoke: async command => { if (command === 'preview_stop') throw Error('already gone'); return { gen: 1 }; } });
  await h.p.ensure(0); const b = h.makeBitmap(); h.p.frames.set(0, { bmp: b });
  h.p.chunks.set('c', { scheduled: true }); h.p.sources.push({ stop() { throw Error('ended'); } });
  await h.p.stop(); assert.equal(h.p.started, false); assert.equal(b.closed, 1);
  assert.equal(h.p.frames.size, 0); assert.equal(h.p.chunks.size, 0); assert.equal(h.p.sources.length, 0);
  await h.p.stop(); assert.equal(h.calls.filter(c => c.command === 'preview_stop').length, 1);
});
await test('poll guards overlap, accepts explicit updates and recovers a lost session', async () => {
  let fail = 'no preview stream';
  const h = harness({ invoke: async command => { if (command === 'preview_update' && fail) throw Error(fail); return { gen: 1 }; } });
  await h.p.poll(); assert.equal(h.calls.length, 0);
  h.p.started = true; h.p.polling = true; await h.p.poll(); assert.equal(h.calls.length, 0);
  await h.p.poll({ seek: true }); assert.equal(h.p.started, true); assert.equal(h.p.polling, false);
  assert.deepEqual(h.calls.map(c => c.command), ['preview_update', 'preview_start']);
  fail = 'engine unavailable'; await h.p.poll(); assert.match(h.errors[0], /engine unavailable/);
  h.p.take(null); h.p.take({ gen: h.p.gen, error: 'render failure' }); assert.equal(h.errors.at(-1), 'render failure');
});
await test('frame fallback, duplicate suppression, eviction and drawing own bitmap lifetimes', async () => {
  const h = harness({ bitmap: false }); h.p.gen = 1;
  await h.p.fetchFrame({ index: 0, time: 0, url: 'frame' });
  assert.equal(h.p.frames.size, 1); assert.equal(h.p.draw(0), true); assert.equal(h.p.draw(0), true);
  assert.equal(h.p.drawn, 1); assert.equal(h.p.draw(99), false); assert.equal(h.o.canvas.width, 2);
  await h.p.fetchFrame({ index: 0, url: 'duplicate' }); assert.equal(h.bitmaps.length, 1);
  h.p.loading.add(2); await h.p.fetchFrame({ index: 2, url: 'loading' }); assert.equal(h.bitmaps.length, 1);
  h.p.loading.clear(); h.p.t = 1;
  for (let i = 1; i <= 61; i++) await h.p.fetchFrame({ index: i, url: String(i) });
  assert.equal(h.p.frames.size, 60); assert.equal(h.p.frames.has(0), false);
  assert.equal(h.p.frames.has(61), true);
  h.p.pending = [{ index: 61 }, { index: 60 }]; h.p.pump(); assert.equal(h.p.pending.length, 0);
});
await test('decoded stale frame is closed without publication', async () => {
  const fetch = deferred(); const h = harness({ fetcher: () => fetch.promise });
  h.p.gen = 1; const pending = h.p.fetchFrame({ index: 0, url: 'old' }); h.p.gen = 2;
  fetch.resolve({ ok: true, blob: async () => ({}) }); await pending;
  assert.equal(h.p.frames.size, 0); assert.equal(h.bitmaps[0].closed, 1);
});
await test('Web Audio context fallback, scheduling and cleanup preserve source ownership', async () => {
  const started = [], options = [];
  class Audio {
    constructor(o) { options.push(o); this.currentTime = 1; this.state = 'running'; this.destination = {}; }
    async decodeAudioData() { return { duration: 2 }; }
    createBufferSource() { return { connect() {}, start: (...a) => started.push(a), stop() {} }; }
  }
  const h = harness({ audio: { webkitAudioContext: Audio } }); const p = h.p;
  assert.equal(p.actx(), p.actx()); assert.equal(options.length, 1); assert.equal(options[0].sampleRate, 48000);
  p.gen = 1; p.playing = true; p.clock = { anchorTl: 0, anchorAudio: 1, rate: 1, latency: 0 };
  p.chunks.set('unloaded', {}); p.chunks.set('silent', { buf: { duration: 2 }, audible: false });
  p.chunks.set('past', { buf: { duration: 1 }, audible: true, start: -5 });
  await p.fetchChunk({ name: 'a', url: 'a', start: 0, end: 2, audible: true });
  assert.equal(started.length, 1); assert.ok(Math.abs(started[0][1] - 0.01) < 1e-9);
  await p.fetchChunk({ name: 'a', url: 'a' }); assert.equal(started.length, 1);
  const src = p.chunks.get('a').src; src.onended(); assert.equal(p.sources.length, 0); assert.equal(p.chunks.get('a').src, null);
  p.chunks.get('a').src = {}; src.onended(); assert.notEqual(p.chunks.get('a').src, null);
  assert.equal(p.nowS(), 1); p.ctx.state = 'suspended'; h.advance(500); assert.equal(p.nowS(), 1.5);
  p.stopSources(); assert.equal(p.chunks.get('a').scheduled, false);
  p.chunks.clear(); p.t = 10;
  for (let i = 0; i < 25; i++) p.chunks.set(String(i), { name: String(i), end: 1, src: i === 0 ? {} : null });
  await p.fetchChunk({ name: 'new', url: 'new', start: 10, end: 12, audible: false });
  assert.equal(p.chunks.size, 2); assert.equal(p.chunks.has('0'), true);
  const noAudio = harness(); await noAudio.p.fetchChunk({ name: 'none' }); assert.equal(noAudio.p.chunks.size, 0);
});
await test('audio HTTP errors and stale errors do not masquerade as valid chunks', async () => {
  const h = harness({ fetcher: async () => ({ ok: false, status: 503 }) }); h.p.ctx = {}; h.p.gen = 1;
  await h.p.fetchChunk({ name: 'bad', url: 'bad' }); assert.match(h.errors[0], /HTTP 503/); assert.equal(h.p.chunks.size, 0);
  const fetch = deferred(); const stale = harness({ fetcher: () => fetch.promise }); stale.p.ctx = {}; stale.p.gen = 1;
  const pending = stale.p.fetchChunk({ name: 'old', url: 'old' }); stale.p.gen = 2; fetch.reject(Error('old')); await pending;
  assert.equal(stale.errors.length, 0);
});
await test('play tolerates failed audio resume; periodic polling stops on pause', async () => {
  class Audio { constructor() { this.state = 'suspended'; } async resume() { throw Error('gesture required'); } }
  const h = harness({ audio: { AudioContext: Audio }, invoke: async () => ({ gen: 1, audio_on: true }) });
  assert.equal(await h.p.play(0, 0), true); assert.equal(h.p.rate, 1);
  assert.equal(h.timers.size, 1); const timer = [...h.timers.values()][0]; assert.equal(timer.ms, 120); await timer.f();
  assert.ok(h.calls.filter(c => c.command === 'preview_update').length >= 2);
  h.p.setRate(0); assert.equal(h.p.playing, false); h.p.tickPoll(); assert.equal(h.timers.size, 0);
  await h.p.setRate(2); assert.equal(h.p.rate, 2); await h.p.setRate(-1); assert.equal(h.p.rate, -1); h.p.pause();
});
await test('scrub reuses cached frames and coalesces pending requests to the latest time', async () => {
  const first = deferred(); let frames = 0;
  const h = harness({ invoke: async (command, args) => command === 'preview_frame' ? (++frames === 1 ? first.promise : { url: 'next', index: Math.round(args.time * 30), ms: 2, cached: false }) : { gen: 1, audio_on: false } });
  await h.p.ensure(0); h.p.frames.set(0, { bmp: h.makeBitmap() }); assert.equal(await h.p.scrub(0), true);
  const pending = h.p.scrub(1); await settle(); await h.p.scrub(2); await h.p.scrub(3);
  first.resolve({ url: 'first', index: 30, ms: 1, cached: false }); await pending; await settle();
  assert.equal(frames, 2); assert.equal(h.p.shown, 90); assert.equal(h.p.scrubBusy, false);
  assert.equal(h.draws.length, 2); // cached zero and latest request; superseded request is never drawn.
  assert.equal(h.bitmaps[1].closed, 1); assert.equal(h.bitmaps[2].closed, 1);
});
await test('scrub cannot overwrite a newer playback epoch', async () => {
  const frame = deferred();
  const h = harness({ invoke: async command => command === 'preview_frame' ? frame.promise : { gen: 1, audio_on: false } });
  await h.p.ensure(0); h.p.playing = true;
  const pending = h.p.scrub(1); await settle(); h.p.epoch = 2;
  frame.resolve({ url: 'stale', index: 30 }); await pending;
  assert.equal(h.draws.length, 0); assert.equal(h.bitmaps[0].closed, 1); assert.equal(h.p.scrubBusy, false);
  const start = deferred(); const slow = harness({ invoke: () => start.promise });
  const scrub = slow.p.scrub(1); slow.p.epoch = 4; start.resolve({ gen: 1 }); assert.equal(await scrub, false);
});
await test('scrub errors release busy state and lost sessions can restart', async () => {
  let error = 'no preview stream';
  const h = harness({ invoke: async command => { if (command === 'preview_frame') throw Error(error); return { gen: 1 }; } });
  await h.p.scrub(1); assert.equal(h.p.started, false); assert.equal(h.errors.length, 0); assert.equal(h.p.scrubBusy, false);
  error = 'decode failure'; await h.p.scrub(2); assert.match(h.errors[0], /decode failure/);
  const unsupported = harness({ invoke: async () => { throw Error('unknown command'); } }); assert.equal(await unsupported.p.scrub(1), false);
  h.p.info = {}; assert.equal(h.p.stats().size, '');
  h.p.info = { width: 2, height: 1, proxies: ['a'], skip: 2, render_fps: 29.98 };
  assert.equal(h.p.stats().size, '2×1'); assert.equal(h.p.stats().proxies, 1); assert.equal(h.p.stats().skip, 2);
});
await test('forward/reverse endpoints and loop boundaries publish the correct playhead', async () => {
  for (const [rate, time, loop, expected, playing] of [
    [1, 9.99, { on: false }, 10 - 1 / 30, false],
    [-1, 0, { on: false }, 0, false],
    [1, 3, { on: true, in: 1, out: 3 }, 1, true],
    [-1, 1, { on: true, in: 1, out: 3 }, 3 - 1 / 30, true],
    [-1, 0, { on: true, in: null, out: null }, 10 - 1 / 30, true],
  ]) {
    const h = harness(); h.p.info = { audio_on: false }; h.p.rate = rate; h.p.t = time;
    h.p.loop = loop; h.p.playing = true; h.p.go(); h.advance(100);
    assert.ok(Math.abs(h.p.t - expected) < 1e-9); assert.equal(h.p.playing, playing);
    if (playing) assert.equal(h.p.buffering, true);
    h.p.pause();
  }
  const h = harness();
  assert.equal(h.av.clock(0, 0, NaN, NaN).latency, 0);
  assert.equal(h.av.frameDue(h.av.clock(0, 0, 1, 0), 1, 0), 0);
  assert.equal(h.av.wrap(2, { on: true, in: 1, out: 1 }, 10), 2);
});
await test('buffering waits for a moving audio clock and bounds the initial chunk wait', async () => {
  for (const latency of [{ outputLatency: 0.04 }, { baseLatency: 0.02 }, {}]) {
    const h = harness(); const p = h.p; p.info = { audio_on: true };
    p.ctx = { currentTime: 0, state: 'running', ...latency }; p.playing = true;
    for (let i = 0; i < 32; i++) p.frames.set(i, { bmp: h.makeBitmap() });
    p.chunks.set('pending', { buf: null });
    p.rebuffer(); h.advance(100); assert.equal(p.buffering, true);
    h.advance(1600); assert.equal(p.buffering, true); // no device clock yet, despite expired chunk wait
    p.ctx.currentTime = 1; h.advance(1); assert.equal(p.buffering, false);
    assert.equal(p.clock.latency, latency.outputLatency || latency.baseLatency || 0);
    p.pause();
  }
  const h = harness(); h.p.info = { audio_on: true }; h.p.ctx = { currentTime: 2, state: 'running' };
  h.p.playing = true; for (let i = 0; i < 32; i++) h.p.frames.set(i, { bmp: h.makeBitmap() });
  h.p.chunks.set('silent', { buf: { duration: 1 }, start: 0, end: 1, audible: false });
  h.p.rebuffer(); h.advance(1); assert.equal(h.p.buffering, false); h.p.pause();
});
await test('direct scrub sizes its canvas and publishes frame metadata', async () => {
  const h = harness({ invoke: async command => command === 'preview_frame' ? { url: 'frame', index: 30, ms: 1.8, cached: true } : { gen: 1 } });
  assert.equal(await h.p.scrub(1), true); assert.equal(h.p.shown, 30); assert.equal(h.o.canvas.width, 2);
  assert.equal(h.states.at(-1)[0], 'scrub'); assert.equal(h.states.at(-1)[1].engine_ms, 2);
  assert.equal(h.states.at(-1)[1].cached, true); assert.equal(h.bitmaps[0].closed, 1);
});
await test('preparation polls until frames arrive and ensure reuses the ready session', async () => {
  const warm = [];
  const h = harness({ invoke: async command => command === 'preview_update' ?
    { gen: 1, audio_on: false, frames: Array.from({ length: 32 }, (_, index) => ({ index, time: index / 30, url: String(index) })) } : { gen: 1, audio_on: false } });
  h.o.onWarm = state => warm.push(state);
  const pending = h.p.prepare(0); await settle();
  for (let i = 0; i < 3 && h.timers.size; i++) {
    const [key, timer] = h.timers.entries().next().value; h.timers.delete(key); timer.f(); await settle();
  }
  await pending; assert.equal(warm.at(-1), 'ready'); assert.equal(h.p.frames.size, 32);
  assert.ok(h.calls.some(c => c.command === 'preview_update'));
  assert.equal(await h.p.ensure(0), true); assert.equal(h.p.warm, null);
  assert.equal(h.calls.filter(c => c.command === 'preview_start').length, 1);
  assert.equal(h.calls.filter(c => c.command === 'preview_stop').length, 0);
  await h.p.stop();
});
await test('preparation cancellation during idle delay stops its session, even after stop failure', async () => {
  const h = harness({ invoke: async command => { if (command === 'preview_stop') throw Error('already stopped'); return { gen: 1 }; } });
  const pending = h.p.prepare(0); await settle();
  const cancelled = h.p.cancelPreparation();
  const [key, timer] = h.timers.entries().next().value; h.timers.delete(key); timer.f();
  await Promise.all([pending, cancelled]); assert.equal(h.p.started, false); assert.equal(h.p.warm, null);
  assert.equal(h.calls.filter(c => c.command === 'preview_update').length, 0);
  const ignored = harness(); ignored.p.playing = true; await ignored.p.prepare(0); assert.equal(ignored.calls.length, 0);
});
await test('preparation engine errors skip readiness and release the failed session', async () => {
  const warm = []; const h = harness({ invoke: async () => ({ gen: 1, error: 'cannot prepare' }) });
  h.o.onWarm = state => warm.push(state); await h.p.prepare(0);
  assert.equal(warm.at(-1), 'skipped'); assert.equal(h.p.started, false);
  assert.equal(h.calls.at(-1).command, 'preview_stop'); assert.equal(h.errors[0], 'cannot prepare');
});
console.log(`${count} lifecycle cases passed`);
