// kimo app window: start-up. The only module: it may await, so it loads the dev mock bridge when not inside the app,
// registers every bridge event listener and runs the start-up sequence. Everything it uses is a global from studio/*.js.
if (!B) B = (await import('../dev/mock-bridge.js')).bridge;
window.__kimoBridge = B;

// ================================================================================================================
// FILE DROP (the OS drops files on the window; Tauri hands us their paths)
let dropDepth = 0;
B.listen('tauri://drag-enter', () => { dropDepth = 1; $('#dropcover').classList.add('on'); $('#bin').classList.add('hot'); });
B.listen('tauri://drag-leave', () => { dropDepth = 0; $('#dropcover').classList.remove('on'); $('#bin').classList.remove('hot'); });
B.listen('tauri://drag-drop', p => {
  $('#dropcover').classList.remove('on'); $('#bin').classList.remove('hot'); dropDepth = 0;
  const paths = p?.paths || [];
  const proj = paths.find(x => /\.kimo$/i.test(x));
  if (proj) return openProject(proj);
  if (S.tab !== 'edit') show('edit');
  importPaths(paths);
});

// ================================================================================================================
// WIRING
B.listen('live-event', onLive);
B.listen('external-event', onExternal);
B.listen('changes', list => { const flash = list.length > S.changes.length ? list[list.length - 1]?.id : null; S.changes = list; renderChanges(flash); });
B.listen('project-changed', m => {
  S.dirty = m?.action !== 'new_project' && m?.action !== 'load_project'; setProjectName(S.app?.project_path);
  loadSoon(m?.actor && m.actor !== 'you' ? 60 : 30); viewerSoon();
});
B.listen('agent-event', onAgentEvent);
B.listen('agent-connection', m => { renderAttached(m.attached || {}); if (m.up && m.client !== currentAgent()) toast(`${actorName(m.client)} connected to this project`); });
B.listen('setup-progress', onProgress);
B.listen('open-file', p => openProject(p));
B.listen('autosaved', m => { const el = $('#tb-auto'); el.hidden = false; el.textContent = 'Autosaved ' + new Date(m.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }); el.title = 'Saved for crash recovery to ' + m.path + '. Use Save to write your own file.'; });
B.listen('engine-status', s => { refresh().then(() => { if (s?.state === 'running') { loadProject(); viewerSoon(); } }); });
addEventListener('resize', () => { if (S.tab === 'edit') renderTimeline(); });

await refresh();
renderChanges(); renderLive(); renderCliCards(); renderAgentPick();
S.changes = await B.invoke('changes_list').catch(() => []); renderChanges();
const st = await setupStatus().catch(() => null);
if (S.app?.engine?.state === 'running') {
  await loadProject(); viewerRefresh();
  const tr = await read('effects', 'list_transitions');
  S.transitions = (Array.isArray(tr) ? tr : tr?.transitions || tr?.kinds || []).map(x => typeof x === 'string' ? x : x.kind || x.name).filter(Boolean).slice(0, 60);
}
refreshCli();
// a crash (PC restart, power cut) left unsaved work: offer it back before anything else
const lost = await B.invoke('recovery_status').catch(() => null);
if (lost) {
  const when = new Date(lost.autosaved_at).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
  const what = lost.project_path ? base(lost.project_path).replace(/\.(kimo|json)$/, '') : 'an unsaved project';
  if (await confirmBox('Restore your unsaved work?', `kimo closed before your last changes to ${what} were saved. Restore the copy autosaved at ${when}?`, 'Restore', 'Discard')) {
    try { await B.invoke('recovery_restore'); S.dirty = true; await refresh(); show('edit'); await loadProject(); viewerSoon(); toast('Restored. Press Save to keep it.'); }
    catch (e) { toast(String(e), 'bad'); }
  } else { B.invoke('recovery_discard'); }
} else if (st && (st.ready || st.done)) maybeWelcome();
setInterval(() => { if (S.app?.engine?.state !== 'running') refresh().then(() => { if (S.app?.engine?.state === 'running') { loadProject(); viewerSoon(); } }); }, 2000);
window.__kimo = { S, P, V, T, show, refresh, loadProject, sendAgent, importPaths, viewerRefresh };
document.body.dataset.ready = '1';
