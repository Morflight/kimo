// Run with node --experimental-vm-modules. The complete original module retains its source offsets.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
const filename = fileURLToPath(new URL('../ui/bridge.js', import.meta.url));
const source = readFileSync(filename, 'utf8');

async function harness(search = '', { tauri = false, replayError = false } = {}) {
  const statuses = [], events = [], streams = [], timers = [], listeners = {}, plays = [];
  class EventSource {
    static CLOSED = 2;
    constructor(url) { this.url = url; this.readyState = 0; this.closed = false; streams.push(this); }
    close() { this.closed = true; }
  }
  const context = vm.createContext({ window: tauri ? { __TAURI__: { event: { listen(name, fn) { listeners[name] = fn; } } } } : {},
    location: { search }, URLSearchParams, EventSource, setTimeout(fn, ms) { timers.push({ fn, ms }); } });
  const mod = new vm.SourceTextModule(source, { context, identifier: filename,
    async importModuleDynamically(specifier) {
      assert.equal(specifier, '../demo/replay.js');
      if (replayError) throw new Error('demo unavailable');
      const replay = new vm.SyntheticModule(['play'], function () {
        this.setExport('play', (onEvent, params) => { plays.push(params.get('scenario')); onEvent({ simulated: true }); });
      }, { context });
      await replay.link(() => {}); await replay.evaluate(); return replay;
    } });
  await mod.link(() => {}); await mod.evaluate();
  const connection = mod.namespace.connect(e => events.push(e), (...s) => statuses.push(s));
  const settle = () => new Promise(resolve => setImmediate(resolve));
  await settle();
  return { connection, statuses, events, streams, timers, listeners, plays, settle };
}

let count = 0;
async function test(name, fn) { await fn(); count++; console.log('ok', name); }
await test('Tauri takes priority; engine events establish and restore liveness', async () => {
  const h = await harness('?feed=sim', { tauri: true });
  assert.equal(h.connection.kind, 'tauri'); assert.equal(h.streams.length, 0);
  assert.deepEqual(h.statuses, [['down', 'listening']]);
  h.listeners['spectator-status']({ payload: 'listening on 7422' });
  assert.deepEqual(h.statuses.at(-1), ['down', 'listening on 7422']);
  h.listeners['engine-event']({ payload: { id: 1 } });
  h.listeners['engine-event']({ payload: { id: 2 } });
  assert.equal(h.statuses.filter(s => s[0] === 'live').length, 1);
  assert.deepEqual(h.events, [{ id: 1 }, { id: 2 }]);
  h.listeners['spectator-status']({ payload: 'listening on 7422' });
  assert.deepEqual(h.statuses.at(-1), ['down', 'reconnecting']);
  h.listeners['spectator-status']({ payload: 'failed' });
  assert.deepEqual(h.statuses.at(-1), ['down', 'failed']);
  h.listeners['spectator-status']({ payload: 'live' });
  assert.deepEqual(h.statuses.at(-1), ['live', 'live']);
  h.listeners['spectator-status']({ payload: 42 });
  assert.deepEqual(h.statuses.at(-1), ['down', '42']);
  h.listeners['engine-event']({ payload: { id: 3 } });
  assert.deepEqual(h.statuses.at(-1), ['live', 'live']);
});
await test('off mode opens no transport', async () => {
  const h = await harness('?feed=off');
  assert.equal(h.connection.kind, 'off'); assert.equal(h.streams.length, 0);
  assert.deepEqual(h.statuses, [['down', 'no feed']]);
});
await test('simulated replay is explicitly labelled and receives query parameters', async () => {
  const h = await harness('?feed=sim&scenario=m16');
  assert.equal(h.connection.kind, 'sim'); assert.deepEqual(h.plays, ['m16']);
  assert.deepEqual(h.events, [{ simulated: true }]);
  assert.deepEqual(h.statuses, [['sim', 'simulated']]);
});
await test('unavailable replay reports no feed', async () => {
  const h = await harness('?feed=sim', { replayError: true });
  assert.deepEqual(h.statuses, [['sim', 'simulated'], ['down', 'no feed']]);
  assert.equal(h.events.length, 0);
});
await test('SSE delivers valid JSON and ignores malformed messages', async () => {
  const h = await harness('?feed=sse&sse=8123');
  assert.equal(h.connection.kind, 'sse'); assert.equal(h.connection.port, '8123');
  const es = h.streams[0]; assert.equal(es.url, 'http://127.0.0.1:8123/events');
  es.onopen(); es.onmessage({ data: '{"call_id":"a"}', lastEventId: 'a /?#' });
  es.onmessage({ data: 'broken', lastEventId: '' });
  assert.deepEqual(JSON.parse(JSON.stringify(h.events)), [{ call_id: 'a' }]);
  assert.deepEqual(h.statuses.at(-1), ['live', 'sse :8123']);
  es.readyState = 0; es.onerror();
  assert.equal(h.timers.length, 0); assert.deepEqual(h.statuses.at(-1), ['down', 'reconnecting']);
  es.readyState = 2; es.onerror(); assert.equal(h.timers[0].ms, 500);
  h.timers.shift().fn();
  assert.equal(h.streams[1].url, 'http://127.0.0.1:8123/events?last=a%20%2F%3F%23');
});
await test('closed SSE retries back off to eight seconds and reset after reconnect', async () => {
  const h = await harness();
  assert.equal(h.streams[0].url, 'http://127.0.0.1:7422/events');
  h.streams[0].onopen();
  for (const expected of [500, 1000, 2000, 4000, 8000, 8000]) {
    const es = h.streams.at(-1); es.readyState = 2; es.onerror();
    assert.equal(h.timers.length, 1); const timer = h.timers.shift();
    assert.equal(timer.ms, expected); timer.fn();
  }
  h.streams.at(-1).onopen(); h.streams.at(-1).readyState = 2; h.streams.at(-1).onerror();
  assert.equal(h.timers.shift().ms, 500);
  assert.equal(h.plays.length, 0);
});
await test('initial forced SSE refusal stays down without simulated data', async () => {
  const h = await harness('?feed=sse'); h.streams[0].onerror(); await h.settle();
  assert.equal(h.streams[0].closed, true);
  assert.deepEqual(h.statuses, [['down', 'sse :7422 refused']]);
  assert.equal(h.timers.length, 0); assert.equal(h.plays.length, 0);
});
await test('automatic initial SSE refusal falls back to labelled replay', async () => {
  const h = await harness(); h.streams[0].onerror(); await h.settle();
  assert.equal(h.streams[0].closed, true);
  assert.deepEqual(h.statuses, [['sim', 'simulated']]);
  assert.deepEqual(h.events, [{ simulated: true }]);
});
console.log(`${count} bridge cases passed`);
