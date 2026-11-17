// kimo app window: viewer.
// Classic script sharing one global scope with the other studio/*.js files (loaded in order by studio.html);
// studio/main.js runs last and wires the bridge events and the start-up sequence.
"use strict";

// ================================================================================================================
// VIEWER (frames and drafts come from the engine through the app, cached per project revision)
const V = { t: 0, dur: 0, fps: 30, rev: null, before: null, draft: null, frames: new Map(), seq: 0, timer: null, raf: 0 };
const vw = $('#viewer'), scrub = $('#vw-scrub'), video = $('#vw-video');
const fidx = t => Math.round(Math.max(0, t) * V.fps);
function vState(s, busy) { vw.dataset.state = s; if (busy) $('#vw-busy-t').textContent = busy; }
function vMedia(m) {
  vw.dataset.media = m;
  $('#vw-img').hidden = m !== 'image' || vw.dataset.view === 'before' || !$('#vw-img').getAttribute('src');
  video.hidden = m !== 'video' || vw.dataset.view === 'before';
}
function vTime() {
  $('#vw-time').innerHTML = V.dur ? `${tcf(V.t)}<span class="of"> / ${tcf(V.dur)}</span>` : tcf(V.t);
  scrub.style.setProperty('--p', V.dur ? `${(100 * V.t / V.dur).toFixed(2)}%` : '0%');
  placePlayhead();
}
function inDraft(t) { const d = V.draft; return d && d.rev === V.rev && t >= d.start && t <= d.end - 1 / V.fps; }
function dropDraft() { stopPlay(); V.draft = null; video.removeAttribute('src'); video.load(); vw.dataset.playing = '0'; vMedia('image'); }
function setView(v) {
  if (v === 'before' && !V.before) v = 'after';
  vw.dataset.view = v;
  $$('#vw-ab button').forEach(b => b.classList.toggle('on', b.dataset.v === v));
  if (v === 'before') { stopPlay(); $('#vw-before').src = V.before.url; }
  $('#vw-before').hidden = v !== 'before';
  const tag = $('#vw-tag'); tag.hidden = v !== 'before';
  if (v === 'before') tag.textContent = `Before · ${tc(V.before.time)}`;
  vMedia(vw.dataset.media);
}
function applyShown(s) {
  V.dur = s.duration; V.fps = s.fps || 30; V.rev = s.rev;
  scrub.max = String(Math.max(0, s.duration)); scrub.step = String(1 / V.fps);
  scrub.disabled = $('#vw-play').disabled = s.empty;
  V.before = s.before || null;
  $('#vw-ab button[data-v=before]').disabled = !V.before;
  if (V.draft && V.draft.rev !== s.rev) dropDraft();
  if (s.empty) {
    $('#vw-img').removeAttribute('src');
    V.t = 0; scrub.value = '0'; vTime(); dropDraft(); setView('after'); $('#vw-rev').textContent = 'empty';
    $('#vw-msg').textContent = 'The project has no clips yet.';
    return vState('empty');
  }
  const f = s.frame;
  V.frames.set(`${f.rev}:${f.index}`, f);
  if (V.frames.size > 300) V.frames.delete(V.frames.keys().next().value);
  $('#vw-img').src = f.url;
  vMedia(inDraft(V.t) && !V.draft.frame_urls?.length ? 'video' : 'image');
  $('#vw-rev').textContent = `${s.width}×${s.height}`;
  vw.style.setProperty('--ar', `${s.width} / ${s.height}`);
  $('#vw-msg').textContent = (f.cached ? 'From cache' : `Rendered in ${f.ms} ms`) + (s.changed ? ' · updated after an edit' : '');
  if (s.changed && vw.dataset.view === 'before') setView('after'); else setView(vw.dataset.view);
  vState('ready');
}
async function viewerRefresh() {
  if (S.app?.engine?.state !== 'running') return;
  const my = ++V.seq;
  vState(vw.dataset.state === 'empty' ? 'empty' : 'rendering', 'Rendering frame…');
  try { const s = await B.invoke('viewer_frame', { time: V.t }); if (my === V.seq) applyShown(s); }
  catch (e) { if (my !== V.seq || String(e) === 'superseded') return; $('#vw-err').textContent = String(e); vState('error'); }
}
function viewerSoon() { clearTimeout(V.timer); V.timer = setTimeout(viewerRefresh, 300); }
scrub.oninput = () => {
  V.t = +scrub.value; vTime();
  if (vw.dataset.view === 'before') setView('after');
  if (inDraft(V.t)) {
    stopPlay();
    if (V.draft.frame_urls?.length) { $('#vw-img').src = seqFrame(V.t); vMedia('image'); } else { video.currentTime = V.t - V.draft.start; vMedia('video'); }
    return;
  }
  const hit = V.frames.get(`${V.rev}:${fidx(V.t)}`);
  if (hit) { $('#vw-img').src = hit.url; vMedia('image'); }
};
scrub.onchange = () => { V.t = +scrub.value; vTime(); if (!inDraft(V.t) && !V.frames.has(`${V.rev}:${fidx(V.t)}`)) viewerRefresh(); };
function seqFrame(t) { const d = V.draft; return d.frame_urls[Math.min(d.frame_urls.length - 1, Math.max(0, Math.round((t - d.start) * (d.fps || V.fps))))]; }
function stopPlay() { cancelAnimationFrame(V.raf); V.raf = 0; if (!video.paused) video.pause(); vw.dataset.playing = '0'; }
function playSeq() {
  const d = V.draft, t0 = performance.now(), from = V.t;
  vw.dataset.playing = '1'; vMedia('image');
  const tick = now => {
    const t = from + (now - t0) / 1000;
    if (t >= d.end - 1 / V.fps || !V.draft) {
      V.t = Math.min(V.dur, d.end - 1 / V.fps); scrub.value = String(V.t); vTime(); stopPlay();
      if (V.t < V.dur - 2 / V.fps && playing) play(); // keep going: next draft chunk
      return;
    }
    V.t = t; scrub.value = String(t); vTime();
    $('#vw-img').src = seqFrame(t);
    V.raf = requestAnimationFrame(tick);
  };
  V.raf = requestAnimationFrame(tick);
}
let playing = false;
async function play() {
  if (V.t >= V.dur - 1 / V.fps) V.t = 0;
  playing = true;
  setView('after');
  if (!inDraft(V.t)) {
    const my = ++V.seq;
    vState('rendering', 'Rendering preview…');
    try {
      const d = await B.invoke('viewer_draft', { start: V.t, seconds: 4 });
      if (my !== V.seq || !playing) return;
      V.draft = d;
      if (d.frame_urls?.length) await Promise.all(d.frame_urls.map(u => { const i = new Image(); i.src = u; return i.decode().catch(() => {}); }));
      else video.src = d.url;
    } catch (e) { if (my === V.seq) { $('#vw-err').textContent = String(e); vState('error'); } playing = false; return; }
  }
  vState('ready');
  if (V.draft.frame_urls?.length) return playSeq();
  vMedia('video');
  try { video.currentTime = V.t - V.draft.start; await video.play(); vw.dataset.playing = '1'; }
  catch (e) { vw.dataset.playing = '0'; playing = false; $('#vw-msg').textContent = "This computer can't play the preview here. Frames still work."; vMedia('image'); }
}
function togglePlay() { if (vw.dataset.playing === '1' || playing && vw.dataset.state === 'rendering') { playing = false; stopPlay(); } else play(); }
video.ontimeupdate = () => { if (vw.dataset.playing === '1' && V.draft) { V.t = V.draft.start + video.currentTime; scrub.value = V.t; vTime(); } };
video.onended = () => { vw.dataset.playing = '0'; if (playing && V.t < V.dur - 2 / V.fps) play(); };
$('#vw-play').onclick = togglePlay;
$$('#vw-ab button').forEach(b => b.onclick = () => setView(b.dataset.v));
$('#vw-retry').onclick = () => viewerRefresh();

// guides (title/action safe, thirds), full screen and frame stepping
function toggleGuide(g) {
  const on = new Set((vw.dataset.guides || '').split(' ').filter(Boolean));
  if (on.has(g)) on.delete(g); else on.add(g);
  vw.dataset.guides = [...on].join(' ');
  for (const [id, k] of [['#vw-safe', 'safe'], ['#vw-grid', 'grid']]) { $(id).classList.toggle('on', on.has(k)); $(id).setAttribute('aria-pressed', on.has(k)); }
}
function fullView() {
  if (document.fullscreenElement) return document.exitFullscreen();
  $('.vw-screen').requestFullscreen?.().catch(e => toast(String(e), 'bad'));
}
const stepFrame = n => seekTo(V.t + n / (P.fps || 30));
$('#vw-safe').onclick = () => toggleGuide('safe');
$('#vw-grid').onclick = () => toggleGuide('grid');
$('#vw-full').onclick = fullView;
$('#vw-prev').onclick = () => stepFrame(-1);
$('#vw-next').onclick = () => stepFrame(1);
