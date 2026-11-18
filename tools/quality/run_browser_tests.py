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
    args = parser.parse_args()
    return run(cases(), args.output, args.timeout)


if __name__ == '__main__':
    raise SystemExit(main())
