import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import vm from 'node:vm';
const filename = fileURLToPath(new URL('../ui/modes.js', import.meta.url));
const module = new vm.SourceTextModule(readFileSync(filename, 'utf8'), {identifier: filename});
await module.link(() => { throw Error('mode definitions must not depend on a transport'); });
await module.evaluate();
const {MODES, inferMode} = module.namespace;
const routes = [
  ['set_color_pipeline', 'color'], ['set_meta', 'edit'], ['load_project', 'batch'],
  ['add_marker', 'edit'], ['burn_in', 'captions'], ['export_video', 'export'],
  ['render_scene3d_sequence', 'export'], ['add_caption', 'captions'], ['undo', 'batch'],
  ['set_motion_path', 'graph'], ['apply_motion_preset', 'graph'], ['solve_constraints', 'graphics'],
  ['preview_subject_match', 'grading'], ['normalize', 'mixing'], ['add_grade', 'grading'],
  ['render_scope', 'color'], ['add_effect', 'effects'], ['set_track_matte', 'roto'],
  ['add_track', 'edit'], ['track_region', 'tracking'], ['add_mesh', 'modelling'],
  ['add_hud', 'graphics'], ['simulate_particles', 'particles'], ['set_light', 'lighting'],
  ['set_camera_keyframe', 'scene3d'], ['set_keyframe', 'edit'], ['add_mask', 'roto'],
  ['scene_add', 'scene3d'], ['analyze_frame', 'grading'], ['trim_clip', 'edit'], ['duplicate_clip', 'edit'],
];
for (const [tool, expected] of routes) assert.equal(inferMode(tool), expected, tool);
assert.deepEqual([...new Set(routes.map(([, mode]) => mode))].sort(), Object.keys(MODES).sort());
console.log('PASS routing covers every workspace and precedence-sensitive tool families');
for (const tool of [undefined, null, '', 0, 42, 'unknown_operation', 'GET_SCOPES', 'get_audio',
  'history_list', 'inspect_mesh', 'camera_info', 'track_metrics', 'transform_ocio_samples', 'text_contrast_series']) {
  assert.equal(inferMode(tool), null, String(tool));
}
console.log('PASS read-only, empty and unknown tool names never switch workspace');
for (const [name, mode] of Object.entries(MODES)) {
  assert.ok(mode.top >= 0 && mode.foot >= 0, name);
  assert.equal(mode.panels.agent.kind, 'agent', name);
  assert.equal(mode.panels.agent.title, 'Agent', name);
  for (const [panel, rectangle] of Object.entries(mode.rects)) {
    assert.ok(mode.panels[panel], `${name}/${panel} has a renderer`);
    assert.equal(rectangle.length, 4);
    assert.ok(rectangle.every(Number.isFinite));
    assert.ok(rectangle[2] > rectangle[0] && rectangle[3] > rectangle[1], `${name}/${panel} has positive area`);
  }
}
assert.equal(MODES.mixing.panels.agent.opts.rowH, 37);
assert.equal(MODES.edit.panels.agent.opts.count, true);
console.log('PASS all workspace rectangles and shared agent defaults are valid');
