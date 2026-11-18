#!/usr/bin/env python3
"""Validate feature links, production inventory and same-change docs/tests."""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).splitlines()

def production(path):
    return (path.startswith('crates/') and '/vendor/' not in path and
            (('/src/' in path and path.endswith(('.rs', '.py', '.js'))) or
             (path.startswith('crates/kimo-gui/ui/') and path.endswith('.js'))))

def check(base=None):
    entries = json.loads((ROOT / 'docs/features/catalog.json').read_text())['features']
    tracked = set(git('ls-files'))
    expected = {p for p in tracked if production(p)}
    owners, errors = {}, []
    changed = set(git('diff', '--name-only', base)) if base else set()
    for entry in entries:
        ident = entry['id']
        docs, tests, sources = (entry[k] for k in ('docs', 'tests', 'sources'))
        if not docs or not tests or not sources:
            errors.append(f'{ident}: docs, tests and sources must all be nonempty')
        for path in docs + tests + sources:
            if not (ROOT / path).is_file():
                errors.append(f'{ident}: missing {path}')
        for path in sources:
            if path in owners:
                errors.append(f'{path}: duplicate owner')
            owners[path] = ident
        if changed.intersection(sources):
            if not changed.intersection(docs):
                errors.append(f'{ident}: changed implementation requires updated feature documentation')
            if not changed.intersection(tests):
                errors.append(f'{ident}: changed implementation requires updated behavioral tests')
    errors.extend(f'unclassified production file: {p}' for p in sorted(expected - owners.keys()))
    errors.extend(f'stale/non-production source: {p}' for p in sorted(owners.keys() - expected))
    return errors

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--base')
    args = parser.parse_args()
    errors = check(args.base)
    for error in errors:
        print('FAIL:', error)
    print(f'Feature contract check: {len(errors)} error(s)')
    raise SystemExit(bool(errors))
