// Protocol assertions for the synthetic fixture, not native engine/keychain/provider acceptance.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
const filename = fileURLToPath(new URL('../ui/dev/mock-bridge.js', import.meta.url));
async function fixture(search = '') {
  const window = {}, context = vm.createContext({ window, location: { search }, URLSearchParams, structuredClone,
    setTimeout: fn => setImmediate(fn) });
  const module = new vm.SourceTextModule(readFileSync(filename, 'utf8'), { context, identifier: filename });
  await module.link(() => {}); await module.evaluate();
  return { bridge: module.namespace.bridge, mock: window.__mock };
}
const plain = value => JSON.parse(JSON.stringify(value));
let count = 0;
async function test(name, fn) { await fn(); count++; console.log('ok', name); }
await test('IPC results are detached from fixture state and unknown commands fail', async () => {
  const { bridge: b, mock } = await fixture();
  const app = await b.invoke('app_state'); app.settings.provider = 'invalid';
  assert.equal(mock.M.settings.provider, 'openai');
  await assert.rejects(b.invoke('not_a_command'), e => String(e).includes('unknown command'));
  const agents = await b.invoke('cli_status'); assert.equal(agents.find(a => a.id === 'codex').installed, false);
  const models = await b.invoke('cli_models', { id: 'claude-code' }); assert.equal(models.models[0].id, 'test-model');
});
await test('CLI fixture setup requires known installed agents for sign-in/registration', async () => {
  const { bridge: b } = await fixture();
  await assert.rejects(b.invoke('cli_register', { id: 'codex' }));
  await assert.rejects(b.invoke('cli_signin', { id: 'codex' }));
  await assert.rejects(b.invoke('cli_install', { id: 'unknown' }));
  await b.invoke('cli_install', { id: 'codex' }); await b.invoke('cli_signin', { id: 'codex' }); await b.invoke('cli_register', { id: 'codex' });
  const codex = (await b.invoke('cli_status')).find(a => a.id === 'codex');
  assert.equal(codex.installed && codex.signed_in && codex.registered, true);
});
await test('agent selection and model changes preserve unrelated settings', async () => {
  const { bridge: b } = await fixture();
  await b.invoke('agent_select', { agent: 'claude-code' });
  await b.invoke('agent_select', { agent: 'codex', model: ' test-model ', effort: ' high ', select: false });
  const settings = (await b.invoke('app_state')).settings;
  assert.equal(settings.agent, 'claude-code'); assert.equal(settings.agent_models.codex, 'test-model'); assert.equal(settings.agent_efforts.codex, 'high');
  await assert.rejects(b.invoke('agent_select', { agent: 'unknown' }));
});
await test('API agent requires a key; status returns hints without the secret', async () => {
  const { bridge: b } = await fixture(); await b.invoke('agent_select', { agent: 'api' });
  await assert.rejects(b.invoke('agent_send', { text: 'captions', turn: 1 }), e => String(e).includes('key'));
  await b.invoke('key_set', { provider: 'openai', key: 'synthetic-secret-1234', remember: true });
  const app = await b.invoke('app_state'); assert.equal(JSON.stringify(app).includes('synthetic-secret-1234'), false);
  assert.equal(app.keys.find(k => k.provider === 'openai').hint, 'syn…1234');
});
await test('agent turns emit current events and ordered undo restores earlier state', async () => {
  const { bridge: b, mock } = await fixture(), events = [], live = [], changes = [], revisions = [];
  b.listen('agent-event', e => events.push(e)); b.listen('live-event', e => live.push(e));
  b.listen('changes', e => changes.push(e)); b.listen('project-changed', e => revisions.push(e));
  await b.invoke('agent_select', { agent: 'claude-code' });
  const reply = await b.invoke('agent_send', { text: 'captions', turn: 7 }); assert.match(reply.reply, /54 → 72/);
  await b.invoke('agent_send', { text: 'cut pauses', turn: 8 });
  assert.equal(mock.M.edit.caption, 72); assert.equal(mock.M.edit.cut, true);
  const ledger = await b.invoke('changes_list'); assert.deepEqual(ledger.map(c => c.depth), [1, 2]);
  assert.equal(ledger[0].actor, 'claude-code'); assert.ok(events.every(e => e.agent === 'claude-code' && [7, 8].includes(e.turn)));
  assert.ok(events.some(e => e.ev.kind === 'tool_start' && e.ev.id && e.ev.input.action));
  assert.ok(events.some(e => e.ev.kind === 'tool_end' && e.ev.ok)); assert.ok(live.some(e => e.phase === 'end' && e.change?.id === 1));
  assert.equal(changes[0].length, 1); assert.equal(changes[1].length, 2); assert.equal(revisions.length, 2);
  await b.invoke('agent_reset'); assert.equal((await b.invoke('changes_list')).length, 2);
  const undo = await b.invoke('change_undo', { id: 1 }); assert.deepEqual(undo.undone, [1, 2]);
  assert.equal(mock.M.edit.caption, 54); assert.equal(mock.M.edit.cut, false);
  assert.equal((await b.invoke('changes_list')).every(c => c.undone), true);
  await assert.rejects(b.invoke('change_undo', { id: 1 }));
});
await test('stop cancels pending synthetic work without adding a change', async () => {
  const { bridge: b } = await fixture();
  const pending = b.invoke('agent_send', { text: 'captions', turn: 1 });
  await b.invoke('agent_stop'); await assert.rejects(pending, e => String(e).includes('stopped'));
  assert.deepEqual(await b.invoke('changes_list'), []);
});
await test('separate engine telemetry is distinct from same-project ledger events', async () => {
  const { bridge: b, mock } = await fixture(), external = [], live = [];
  b.listen('external-event', e => external.push(e)); b.listen('live-event', e => live.push(e));
  await mock.engineCall({ kind: 'external', conn: 2 }, 'timeline', 'split_clip', {}, {});
  assert.deepEqual(plain(external.map(e => [e.conn, e.ev.phase])), [[2, 'start'], [2, 'complete']]); assert.equal(live.length, 0);
  assert.deepEqual(await b.invoke('changes_list'), []);
});
await test('setup readiness follows required parts and onboarding is recorded', async () => {
  const normal = await fixture();
  assert.equal((await normal.bridge.invoke('setup_status')).ready, true);
  const { bridge: b } = await fixture('?setup=show'), progress = [];
  b.listen('setup-progress', p => progress.push(p));
  assert.equal((await b.invoke('setup_status')).ready, false);
  await b.invoke('setup_start');
  const status = await b.invoke('setup_status');
  assert.equal(status.ready, true); assert.ok(status.parts.every(p => p.satisfied));
  assert.ok(progress.some(p => p.phase === 'downloading' && p.bytes_per_s > 0 && p.total > p.done));
  await b.invoke('onboarded_set'); assert.equal((await b.invoke('app_state')).settings.onboarded, true);
});
console.log(`${count} synthetic bridge cases passed`);
