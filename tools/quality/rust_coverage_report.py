"""Run cargo-llvm-cov, retrying Windows' argv limit through LLVM's response file."""
import argparse
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile


def long_command(stderr):
    if 'os error 206' not in stderr:
        return None
    match = re.search(r'could not execute process `([^`]+)` \(never executed\)', stderr)
    if not match:
        return None
    try:
        command = shlex.split(match[1])
    except ValueError:
        return None
    if len(command) < 2 or command[0].replace('\\', '/').split('/')[-1] != 'llvm-cov.exe' or command[1] != 'export':
        return None
    return command


def response_text(args, windows):
    return subprocess.list2cmdline(args) if windows else shlex.join(args)


def report(output, args):
    result = subprocess.run(['cargo', 'llvm-cov', 'report', '--json', '--output-path', str(output), *args], capture_output=True, text=True)
    command = long_command(result.stderr) if result.returncode else None
    sys.stdout.write(result.stdout)
    if command is None:
        sys.stderr.write(result.stderr)
        return result.returncode
    # The original exporter never started. Preserve EVERY argument and object;
    # only its transport changes, with no new coverage filter or denominator.
    # llvm-cov dispatches its subcommand before expanding response-file options.
    with tempfile.TemporaryDirectory(prefix='kimo-llvm-response-') as folder:
        path = Path(folder) / 'export.rsp'
        path.write_text(response_text(command[2:], os.name == 'nt'), encoding='utf-8')
        print('Retrying LLVM export with all original arguments in a response file.', file=sys.stderr)
        with Path(output).open('wb') as stream:
            return subprocess.run([command[0], command[1], '@' + str(path)], stdout=stream).returncode


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output-path', required=True)
    options, extra = parser.parse_known_args()
    raise SystemExit(report(options.output_path, extra))
