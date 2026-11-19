"""Require exact line/branch coverage and inventory every first-party Python helper."""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def in_scope(path):
    return path.endswith('.py') and (
        (path.startswith('crates/') and '/src/' in path and '/vendor/' not in path)
        or path.startswith('tools/quality/'))


def check(report):
    tracked = subprocess.check_output(['git', 'ls-files'], cwd=ROOT, text=True).splitlines()
    expected = {path for path in tracked if in_scope(path)}
    data = json.loads(Path(report).read_text())
    errors = []
    if not expected:
        errors.append('empty Python inventory')
    if not data.get('meta', {}).get('branch_coverage'):
        errors.append('branch coverage was not collected')
    files = {name.replace('\\', '/'): item for name, item in data.get('files', {}).items()}
    for name in sorted(expected):
        if name not in files:
            errors.append(f'{name}: absent from coverage report')
            continue
        summary = files[name].get('summary', {})
        for total, covered, missing in [('num_statements', 'covered_lines', 'missing_lines'),
                                        ('num_branches', 'covered_branches', 'missing_branches')]:
            counts = [summary.get(key) for key in (total, covered, missing)]
            if any(type(value) is not int or value < 0 for value in counts):
                errors.append(f'{name}: invalid/missing {total} counts')
            elif counts[0] != counts[1] or counts[2] != 0:
                errors.append(f'{name}: {covered} {counts[1]}/{counts[0]}, missing {counts[2]}')
    return errors


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('report')
    args = parser.parse_args()
    errors = check(args.report)
    for error in errors:
        print('FAIL:', error)
    raise SystemExit(bool(errors))
