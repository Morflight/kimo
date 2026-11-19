"""Run isolated Chromium UI proofs; preserve every failure and only stop owned children."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
TESTS = ROOT / 'crates/kimo-gui/tests'


def cases():
    scripts = sorted(TESTS.glob('*_test.py')) + [TESTS / 'idle_proof.py']
    if len(scripts) <= 1 or not all(script.is_file() for script in scripts):
        raise ValueError('missing browser test inventory')
    result = [(script.stem, script, [], {}, 'report' if script.name == 'coverage_test.py' else 'test')
              for script in scripts]
    result.extend([
        ('studio_ui_dark', TESTS / 'studio_ui_test.py', [], {'KIMO_SCHEME': 'dark'}, 'test'),
        ('reconnect_refused', TESTS / 'reconnect_test.py', ['--refuse', '2'], {}, 'test'),
        ('reconnect_no_replay', TESTS / 'reconnect_test.py', ['--no-replay'], {}, 'test'),
    ])
    return result


def coverage_cases(output):
    # Additional instrumented functional run. The separate acceptance job still
    # runs EVERY case, including timing proofs, without profiler overhead.
    functional = {'app_interaction_test', 'cov_a_test', 'cov_b_test', 'cov_c_test', 'router_test',
                  'studio_draft_test', 'studio_recovery_test', 'panels_canvas_test', 'studio_actions_test', 'studio_ui_test', 'studio_ui_dark', 'playback_ui_test',
                  'preview_rev_test', 'proxy_badge_test', 'feed_gap_test',
                  'keyframe_test', 'ruler_test', 'transition_test', 'flash_edit_scope_test', 'frame_step_precision_test'}
    wrapper = ROOT / 'tools/quality/browser_coverage.py'
    return [(name, wrapper, ['--output', str(Path(output).resolve()), str(script), *args], env, kind)
            for name, script, args, env, kind in cases()
            if name in functional or name.endswith('_row_test')]


def run(selected, output, timeout):
    if os.name != 'posix':
        raise RuntimeError('browser proof runner requires POSIX process groups (Linux CI)')
    if not selected or timeout <= 0:
        raise ValueError('nonempty suite and positive timeout required')
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    results = []
    for name, script, args, additions, kind in selected:
        env = dict(os.environ)
        env.pop('KIMO_SHOTS', None)
        env.update(additions)
        env['PYTHONUNBUFFERED'] = '1'
        log_path = output / (name + '.log')
        with log_path.open('w') as log:
            try:
                child = subprocess.Popen([sys.executable, str(script), *args], cwd=ROOT, env=env,
                                         stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    code = child.wait(timeout=timeout)
                except subprocess.TimeoutExpired:
                    # The new session belongs to this test, including its browser and loopback feed.
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
                    code = 'timeout'
            except OSError as error:
                log.write(str(error) + '\n')
                code = 'launch-error'
        if code == 0 and not log_path.stat().st_size:
            code = 'empty-log'
        row = dict(name=name, kind=kind, exit=code, log=log_path.name)
        results.append(row)
        print(json.dumps(row), flush=True)
        (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    return any(row['exit'] != 0 for row in results)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    parser.add_argument('--timeout', type=float, default=300)
    parser.add_argument('--coverage-output', help='run additional functional proofs with Chromium V8 profiling')
    args = parser.parse_args()
    selected = coverage_cases(args.coverage_output) if args.coverage_output else cases()
    return run(selected, args.output, args.timeout)


if __name__ == '__main__':
    raise SystemExit(main())
