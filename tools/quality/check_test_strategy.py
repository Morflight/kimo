#!/usr/bin/env python3
"""Enforce docs/TESTING_STRATEGY.md mechanically.

- The fast nextest profile excludes exactly the tests in .config/heavy-tests.json, in register order.
- The heavy profile runs every test (no default filter).
- A triaged heavy test has a known category and names an existing fast test that covers the same
  behavior on tiny media; that fast test cannot itself be registered as heavy.
- Untriaged legacy entries can only decrease from the 2026-11-19 baseline.
"""
import json
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
CATEGORIES = {'accuracy', 'performance', 'visual-dump', 'pipeline-at-scale'}
# Heavy tests inherited untriaged on 2026-11-19. Lower it as tests are triaged; never raise it.
LEGACY_UNTRIAGED_MAX = 151


def entry_filter(entry):
    return f"(binary_id({entry['binary']}) & test(={entry['test']}))"


def expected_filter(tests):
    return 'not (\n' + ' |\n'.join('  ' + entry_filter(e) for e in tests) + '\n)\n'


def test_defined(root, name):
    pattern = re.compile(r'\bfn\s+' + re.escape(name) + r'\s*\(')
    return any(pattern.search(path.read_text(encoding='utf-8', errors='replace'))
               for path in sorted((root / 'crates').glob('**/*.rs')) if 'target' not in path.parts)


def check_entries(root, tests):
    errors, keys, untriaged, counterparts = [], [], 0, []
    for index, entry in enumerate(tests):
        key = (entry.get('binary'), entry.get('test'))
        if not all(isinstance(part, str) and part for part in key):
            errors.append(f'entry {index}: binary and test are required strings')
            continue
        name = f'{key[0]}::{key[1]}'
        if key in keys:
            errors.append(f'{name}: duplicate entry')
        keys.append(key)
        category = entry.get('category')
        if category == 'untriaged':
            untriaged += 1
        elif category not in CATEGORIES:
            errors.append(f'{name}: category must be one of {sorted(CATEGORIES)} (or untriaged legacy)')
        else:
            counterparts.append((name, entry.get('fast_test')))
    heavy_names = {test for _, test in keys}
    for name, fast in counterparts:
        if not isinstance(fast, str) or not fast:
            errors.append(f'{name}: fast_test must name the fast tiny-media test covering this behavior')
        elif fast.rsplit('::', 1)[-1] in heavy_names:
            errors.append(f'{name}: fast_test {fast} is itself registered as heavy')
        elif not test_defined(root, fast.rsplit('::', 1)[-1]):
            errors.append(f'{name}: fast_test {fast} is not defined under crates/')
    if keys != sorted(keys):
        errors.append('register entries must be sorted by binary then test')
    if untriaged > LEGACY_UNTRIAGED_MAX:
        errors.append(f'{untriaged} untriaged heavy tests exceed the legacy baseline {LEGACY_UNTRIAGED_MAX}: '
                      'new heavy tests must be triaged with a category and a fast_test')
    return errors


def check(root=ROOT):
    register = json.loads((root / '.config/heavy-tests.json').read_text(encoding='utf-8'))
    tests = register.get('tests') if isinstance(register, dict) else None
    if not isinstance(tests, list) or not all(isinstance(e, dict) for e in tests):
        return ['.config/heavy-tests.json: "tests" must be a list of objects']
    errors = check_entries(root, tests)
    profiles = tomllib.loads((root / '.config/nextest.toml').read_text(encoding='utf-8')).get('profile', {})
    expected = expected_filter([e for e in tests if isinstance(e.get('binary'), str) and isinstance(e.get('test'), str)])
    if profiles.get('fast', {}).get('default-filter') != expected:
        errors.append('.config/nextest.toml [profile.fast] default-filter must exclude exactly the register:\n'
                      'default-filter = """\n' + expected + '"""')
    if 'heavy' not in profiles or 'default-filter' in profiles['heavy']:
        errors.append('.config/nextest.toml must define [profile.heavy] without a default-filter (it runs every test)')
    return errors


def main():
    errors = check()
    for error in errors:
        print('FAIL:', error)
    if not errors:
        print('Test strategy check: 0 error(s)')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())
