"""Strict test-log validation. Legacy known-failure lists never waive failures.

Interface retained: compare_failures.py LOG UNUSED_BASELINE OUTPUT_FAILURES
The caller must ALSO propagate cargo's real exit status (a log is not a receipt).
"""
import re
import sys
from pathlib import Path


def failures(log):
    return set(re.findall(r'^test (\S+) \.\.\. FAILED$', log, re.M))


def validate(log):
    summaries = re.findall(r'^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored', log, re.M)
    errors = []
    if not summaries or sum(int(row[1]) for row in summaries) == 0:
        errors.append('no executed tests with completion summaries')
    if any(state != 'ok' or int(failed) or int(ignored) for state, _, failed, ignored in summaries):
        errors.append('failed or ignored tests')
    if failures(log) or re.search(r'^(error(?:\[.*?\])?:|fatal:)', log, re.M):
        errors.append('test/compiler/runtime error')
    return errors


def main():
    log_path, _, out_path = sys.argv[1:]
    log = Path(log_path).read_text(encoding='utf-8', errors='replace')
    Path(out_path).write_text(''.join(f'{t}\n' for t in sorted(failures(log))))
    errors = validate(log)
    for error in errors:
        print('FAIL:', error)
    if not errors:
        print('Completed tests passed; no failures or ignored tests')
    return bool(errors)


if __name__ == '__main__':
    raise SystemExit(main())
