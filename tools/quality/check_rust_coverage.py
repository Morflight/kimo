#!/usr/bin/env python3
"""Fail unless every first-party Rust runtime file appears and is fully covered."""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]

def check(reports, prefix):
    expected = {p for p in subprocess.check_output(['git', 'ls-files'], cwd=ROOT, text=True).splitlines()
                if p.startswith(prefix) and '/src/' in p and p.endswith('.rs')}
    measured, errors = {}, []
    for report in reports:
        for data in json.loads(Path(report).read_text())['data']:
            for file in data['files']:
                name = file['filename'].replace('\\', '/')
                if '/crates/' in name:
                    name = 'crates/' + name.split('/crates/', 1)[1]
                if name in expected:
                    measured[name] = file['summary']
    for name in sorted(expected):
        if name not in measured:
            errors.append(f'{name}: absent from coverage report')
            continue
        summary = measured[name]
        for metric in ['lines', 'functions', 'regions', 'branches']:
            if metric not in summary:
                if metric == 'branches':
                    continue  # Stable Rust LLVM collector may not emit branch counters.
                errors.append(f'{name}: missing {metric} counts')
                continue
            counts = summary[metric]
            if counts['covered'] != counts['count']:
                errors.append(f'{name}: {metric} {counts["covered"]}/{counts["count"]}')
    if not expected:
        errors.append('empty production inventory')
    return errors

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--prefix', required=True)
    parser.add_argument('reports', nargs='+')
    args = parser.parse_args()
    errors = check(args.reports, args.prefix)
    for error in errors:
        print('FAIL:', error)
    raise SystemExit(bool(errors))
