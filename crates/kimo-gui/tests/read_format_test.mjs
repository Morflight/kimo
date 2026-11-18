// Exercise complete ES modules, including their real cyclic router imports.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import vm from 'node:vm';
const directory = fileURLToPath(new URL('../ui/', import.meta.url));
const context = vm.createContext({performance: {now: () => 1000}}), modules = new Map();
function get(filename) {
  if (!modules.has(filename)) modules.set(filename, new vm.SourceTextModule(readFileSync(filename, 'utf8'), {identifier: filename, context}));
  return modules.get(filename);
}
const root = get(resolve(directory, 'reads.js'));
await root.link((name, parent) => get(resolve(dirname(parent.identifier), name)));
await root.evaluate();
const {short, genericRows, readHead, readRows, callRows, colourRows, scopeLine, CHIP} = root.namespace;
const plain = value => JSON.parse(JSON.stringify(value));
for (const [input, key, expected] of [
  [null, '', null], [undefined, '', null], [true, '', 'yes'], [false, '', 'no'],
  [NaN, '', null], [Infinity, '', null], [0.00001, '', '1.0e-5'], [1.23456, '', '1.235'],
  [1234567.8, '', '1234568'], [0.5, 'opacity', '50%'], [2048, 'memory_bytes', '2 KB'],
  [1024 ** 3, 'memory', '1 GB'], [0, 'bytes', '0 B'], ['', '', null], ['clip123', '', null],
  ['/long/private/path/my-footage.mp4', '', 'my-footage.mp4'],
  [[], '', 'none'], [[1, 2, 3], '', '1 2 3'], [['red', 'green', 'blue', 'amber'], '', 'red, green, blue +1'],
  [[{name: 'First'}, {kind: 'Second'}, {label: 'Third'}], '', '3 · First, Second…'],
  [[{code: 'Named'}, {name: 'clip12'}], '', '2 · Named'], [[null, 'clip12'], '', '×2'],
  [[true, false], '', '×2'], [{}, '', 'none'], [{a: 1}, '', '1 entry'], [{a: 1, b: 2}, '', '2 entries'],
  [Symbol('unsupported'), '', null],
]) assert.equal(short(input, key), expected, `short ${String(input)}`);
assert.equal(short('a'.repeat(30)).length, 26);
console.log('PASS concise values preserve units and hide internal identifiers/full paths');
assert.deepEqual(plain(genericRows(null)), []);
assert.deepEqual(plain(genericRows(3)), [['Result', '3']]);
assert.deepEqual(plain(genericRows('clip123')), []);
assert.deepEqual(plain(genericRows([{}, {}])), [['Items', '2']]);
assert.deepEqual(plain(genericRows(['one', 'two'])), [['Items', '2 · one, two']]);
assert.deepEqual(plain(genericRows({})), [['Result', 'empty']]);
assert.deepEqual(plain(genericRows({map: {a: 1}, array: [1, 2], null_value: null, score: 2, clip_id: 'clip1', prose: 'x'.repeat(41)}, 3)),
  [['Score', '2'], ['Array', '1 2'], ['Map', '1 entry']]);
console.log('PASS generic cards prioritize scalar results and cap visible rows');
assert.equal(readHead('edit_check', {threshold: 0.123}), 'threshold 0.12 · edit_check');
assert.equal(readHead('edit_check', null), 'edit_check');
assert.equal(readHead('other', {}), 'other');
assert.deepEqual(plain(readRows('unknown', {score: 4})), [['Score', '4']]);
assert.deepEqual(plain(readRows('toString', {score: 4})), [['Score', '4']]);
assert.deepEqual(plain(readRows('get_scopes', {luma_mean: 0.2, avg_rgb: [0.1, 0.2, 0.3], clipped_whites_pct: 12.345, crushed_blacks_pct: 0})),
  [['Luma mean', '0.2'], ['Avg RGB', '0.1 0.2 0.3'], ['Clipped whites', '12.3%'], ['Crushed blacks', '0%']]);
assert.deepEqual(plain(readRows('validate_captions', {clean: false, profile: 'broadcast', groups: [1, 2], skipped: []})),
  [['Captions', 'issues'], ['Profile', 'broadcast'], ['Groups', '2'], ['Skipped checks', '0']]);
assert.deepEqual(plain(readRows('validate_captions', {clean: true, legibility: {min_cap_frac: 0.02, min_cap_px: 21.6, min_contrast: 4.5, max_cps: 12}})),
  [['Captions', 'clean'], ['Min caps', '21.6 px · 2%'], ['Lowest contrast', '4.50:1'], ['Max reading', '12.0 cps']]);
console.log('PASS scope and caption readouts retain their measurement semantics');
assert.equal(CHIP.add_text_animator({}), 'animator'); assert.equal(CHIP.add_text_animator({kind: 'wave'}), 'wave');
assert.equal(CHIP.add_text_animator({preset: 'rise'}), 'rise'); assert.equal(CHIP.set_text_path(), 'on path');
assert.equal(CHIP.set_text_style(), 'styled'); assert.equal(CHIP.set_text_weight({}), 'weight'); assert.equal(CHIP.set_text_weight({weight: 700}), 'w700');
assert.deepEqual(plain(callRows('set_text_path', {points: [[0, 0], [1, 1]], start: 0, flip: false}, {}, null, 'Title')),
  [['Clip', 'Title', false], ['Text path', '2 points', true], ['Start', '0', false], ['Flip', 'no', false]]);
assert.deepEqual(plain(callRows('set_text_path', {remove: true})), [['Text path', 'removed', true]]);
assert.deepEqual(plain(callRows('set_text_path', {shape: 'circle'})), [['Text path', 'circle', true]]);
assert.deepEqual(plain(callRows('set_text_path', {})), [['Text path', 'set', true]]);
assert.deepEqual(plain(callRows('remove_track_matte', {})), [['Track matte', 'removed', false]]);
assert.deepEqual(plain(callRows('set_audio_effect_key', {coefficient: 'gain', value: 0.5, time: 2, effect_index: 0})),
  [['Key gain', '0.5 @ 2s', false], ['Effect', '#0', false]]);
assert.deepEqual(plain(callRows('set_text_weight', {weight: 500}, {}, {clip: {'text_fx.weight': 500}})), [['Weight', '500', true], ['Props', '1 changed', false]]);
assert.deepEqual(plain(callRows('unknown', {clip_id: 'clip1', path: '/private', empty: [], gain: 1}, {score: 2})), [['Gain', '1', false], ['Score', '2', false]]);
console.log('PASS mutation rows show names and meaningful parameters with emphasis preserved');
assert.deepEqual(plain(colourRows('pick_white_balance', {}, {gains: [1, 1.1, 0.9]})), [['WB R', '1'], ['WB G', '1.1'], ['WB B', '0.9']]);
assert.deepEqual(plain(colourRows('pick_key_color', {}, {sampled_rgb: [0.1, 0.2, 0.3]})), [['Key R', '0.1'], ['Key G', '0.2'], ['Key B', '0.3']]);
assert.deepEqual(plain(colourRows('match_color', {}, {src_mean: [1, 1, 1], ref_mean: [0, 0, 0]}, 'Reference')), [['Match', 'colour'], ['Ref', 'Reference'], ['Src luma', '1'], ['Ref luma', '0']]);
assert.deepEqual(plain(colourRows('match_grade', {}, {source_lab_mean: [50, 1, 2], reference_lab_mean: [60, 3, 4]}, 'Reference')), [['Match', 'Lab'], ['Ref', 'Reference'], ['Src L', '50'], ['Ref L', '60']]);
assert.deepEqual(plain(colourRows('unknown', {}, null)), []);
for (const invalid of [null, [], 3]) assert.equal(scopeLine('get_scopes', invalid), null);
assert.equal(scopeLine('unknown', {}), null); assert.equal(scopeLine('get_scopes', {}), null);
assert.equal(scopeLine('pick_white_balance', {gains: [1, 2, 3]}), 'pick_white_balance · WB gains 1 2 3');
assert.equal(scopeLine('pick_key_color', {sampled_rgb: [1, 0, 0]}), 'pick_key_color · key 1 0 0');
assert.equal(scopeLine('describe_frame', {frame_stats: {clipped_white_pct: 1, clipped_black_pct: 2}}), 'describe_frame · clip 1% · crush 2%');
assert.equal(scopeLine('match_color', {src_mean: [1, 2, 3], ref_mean: [4, 5, 6]}), 'match_color · mean 1 2 3 → 4 5 6');
assert.equal(scopeLine('match_grade', {source_lab_mean: [1, 2, 3], reference_lab_mean: [4, 5, 6]}), 'match_grade · Lab 1 2 3 → 4 5 6');
console.log('PASS color summaries distinguish RGB, luminance and Lab measurements');
const readContracts = [
  ['analyze_color', {mean:[0.1,0.2,0.3,0.4],min:[0,0,0,0.1],max:[1,1,1,0.9],bins:256}, [['Mean RGB','0.1 0.2 0.3'],['Luma mean','0.4'],['Luma min / max','0.1 / 0.9'],['Bins','256']]],
  ['describe_frame', {canvas:{width:1920,height:1080},frame_stats:{clipped_white_pct:0,clipped_black_pct:2,dominant_colors:[{hex:'#fff'},{hex:'#000'}]}}, [['Canvas','1920×1080'],['Clipped whites','0%'],['Clipped blacks','2%'],['Dominant','#fff #000']]],
  ['cache_stats', {entries:3,bytes:1024,cap_bytes:1048576,hit_rate:0.5,evictions:2}, [['Frames','3'],['Used','1 KB of 1 MB'],['Hit rate','50%'],['Evictions','2']]],
  ['cache_stats', {}, [['Used','0 B of 0 B'],['Hit rate','0%']]],
  ['read_pixels', {count:16,data:[1,0,0,1],width:2,height:2}, [['Pixels','4'],['First RGB','1 0 0'],['Size','2×2']]],
  ['get_project', {assets:{a:{},b:{}},tracks:[{}],duration:2.125,width:640,height:480}, [['Assets','2'],['Tracks','1'],['Duration','2.13 s'],['Canvas','640×480']]],
  ['get_project', {tracks:{a:{},b:{}}}, [['Tracks','2']]],
  ['check_environment', {ok:false,fixes:['install'],ffmpeg:{version:'8.0'}}, [['Demo ready','no'],['Fixes','1'],['ffmpeg','8.0']]],
  ['kimo_info', {version:'1',backends:{cpu:{available:true},gpu:{available:false},absent:null}}, [['Version','1'],['Backends','cpu'],['Version','1'],['Backends','3 entries']]],
  ['kimo_info', {backends:{}}, [['Backends','none'],['Backends','none']]],
  ['list_edit_points', {points:[0,1.234,2]}, [['Edit points','3 · 0s 1.23s 2s']]],
  ['list_markers', {markers:[{name:'Intro',time:1.2},{}]}, [['Markers','2 · Intro 1.2s, 0s']]],
  ['list_markers', {markers:[]}, [['Markers','0']]],
  ['evaluate_hlg_reference_display', {display_rgb_nits:[1.4,2.6,4],gamma:1.2,gamut:'rec2020'}, [['Display','1 3 4 nits'],['Gamma','1.2'],['Gamut','rec2020']]],
  ['history_list', {undo:[{},{}],next_undo:'Split clip',redo:[],checkpoints:[{}]}, [['Undo','2 steps'],['Next undo','Split clip'],['Redo','0'],['Checkpoints','1']]],
  ['snap_time', {time:1.234,snapped:false}, [['Time','1.23 s'],['Snapped','no']]],
  ['session_stats', {calls_total:5,errors_total:1,bytes_returned_total:2048}, [['Calls','5'],['Errors','1'],['Returned','2 KB']]],
  ['inspect_color_pipeline', {default_media_input_gamut:'rec709',default_media_input_transfer:'srgb',working_space:'linear',configured_output_gamut:'rec709',configured_output_transfer:'bt709'}, [['Input','rec709 srgb'],['Working','linear'],['Output','rec709 bt709'],['Default media input gamut','rec709']]],
  ['inspect_color_pipeline', {working:'linear'}, [['Working','linear'],['Working','linear']]],
  ['inspect_color_pipeline', {working_gamut:'aces'}, [['Working','aces'],['Working gamut','aces']]],
  ['saliency_map', {empty:true}, [['Saliency','flat frame, none']]],
];
for (const [tool, result, expected] of readContracts) assert.deepEqual(plain(readRows(tool, result)), expected, tool);
assert.deepEqual(plain(callRows('set_track_matte', {mode:'luma',luma_space:'linear',hide_source:false})), [['Track matte','luma (linear)',false],['Matte visible','yes',false]]);
assert.deepEqual(plain(callRows('set_track_matte', {})), [['Track matte','alpha',false],['Matte visible','no',false]]);
assert.deepEqual(plain(callRows('set_text_style', {font_size:20,fill:[1,0,0]}, {}, {clip:{other:1}})), [['Text style','applied',true],['Font size','20',false],['Fill','1 0 0',false]]);
console.log('PASS project, environment, cache, history and color-pipeline cards retain counts and units');
for (const tool of ['constructor', 'toString', '__proto__']) {
  assert.deepEqual(plain(callRows(tool, {gain:2}, {score:3})), [['Gain','2',false],['Score','3',false]], tool);
  assert.deepEqual(plain(readRows(tool, {score:3})), [['Score','3']], tool);
}
assert.deepEqual(plain(readRows('list_markers', {markers:[null]})), [['Markers','×1']]);
console.log('PASS unknown prototype-like tool names use generic rows and malformed marker entries remain readable');
for (const tool of ['get_scopes','contrast_ratio','edit_check','av_offset','text_contrast_series',
  'query_safe_margins','analyze_color','describe_frame','read_pixels','get_project','check_environment',
  'kimo_info','list_edit_points','list_markers','evaluate_hlg_reference_display','history_list',
  'snap_time','inspect_color_pipeline','saliency_map','frozen_frames','focus_series','render_flow','validate_captions']) {
  assert.deepEqual(plain(readRows(tool, {})), [['Result','empty']], `${tool}: absent measurements must not invent values`);
}
assert.deepEqual(plain(readRows('session_stats', {})), [['Returned','0 B']]);
assert.deepEqual(plain(readRows('get_scopes', {percentiles:[50],luma_percentiles:[0.25]})), [['Luma p50','0.25']]);
assert.deepEqual(plain(readRows('get_scopes', {zone_clipped_whites_max_pct:2,zone_crushed_blacks_max_pct:3})), [['Zone max clip/crush','2% / 3%','wrap']]);
assert.deepEqual(plain(readRows('analyze_color', {min:[0.1],max:[0.9]})), [['Luma min / max','0.1 / 0.9']]);
assert.deepEqual(plain(readRows('describe_frame', {frame_stats:{dominant_colors:[{}, {hex:'#abc'}]}})), [['Dominant','#abc']]);
assert.deepEqual(plain(readRows('query_safe_margins', {overlaps:[],worst_edge_px:0})), [['Max outside safe','no safe zone','wrap'],['Max in avoid','no avoid zone','wrap'],['Worst edge','none crossed','wrap']]);
assert.deepEqual(plain(readRows('query_safe_margins', {overlaps:[]})), [['Max outside safe','no safe zone','wrap'],['Max in avoid','no avoid zone','wrap'],['Worst edge','no safe zone','wrap']]);
assert.deepEqual(plain(readRows('query_safe_margins', {zones:[null,{name:'title'},{name:'action'}],estimated:false,source:'standard'})), [['Zones','3 · , title…','wrap'],['Estimated','no'],['Source','standard','wrap']]);
assert.deepEqual(plain(readRows('av_offset', {best_lag_ms:0,best_r:1})), [['Best lag','0 ms','stack'],['r there / at 0','1.000 / —','wrap']]);
assert.deepEqual(plain(readRows('av_offset', {best_lag_ms:null,onset_max:0})), [['Best lag','none · no audio onsets','stack']]);
assert.deepEqual(plain(readRows('av_offset', {best_lag_ms:null})), [['Best lag','none','stack']]);
assert.deepEqual(plain(callRows('add_text_animator', {wiggly:true})), [['Animator','custom',true],['Wiggly','-1..1 · 2/s',false]]);
assert.deepEqual(plain(callRows('set_text_weight', {}, {}, {clip:7})), [['Weight','default',true]]);
assert.deepEqual(plain(callRows('set_audio_effect_key', {})), []);
assert.deepEqual(plain(callRows('validate_captions', {})), []);
assert.equal(scopeLine('analyze_color', {mean:[0,1,0,0.72],min:[0],max:[1]}), 'analyze_color · mean 0 1 0 · luma 0.72 · range 0–1');
for (const tool of ['analyze_color','describe_frame','pick_white_balance','pick_key_color','match_color','match_grade']) assert.equal(scopeLine(tool, {}), null, tool);
console.log('PASS incomplete measurements stay absent and documented empty/default summaries remain explicit');
