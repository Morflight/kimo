import contextlib
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch
from test_controls import module

runner = module('browser_runner', 'tools/quality/run_browser_tests.py')


class BrowserRunner(unittest.TestCase):
    def test_inventory_includes_all_proofs_and_variants(self):
        cases = runner.cases()
        self.assertEqual(len(cases), len(list(runner.TESTS.glob('*_test.py'))) + 4)
        self.assertEqual(len({row[0] for row in cases}), len(cases))
        self.assertEqual([row[0] for row in cases if row[4] == 'report'], ['coverage_test'])
        self.assertIn('studio_ui_dark', [row[0] for row in cases])
        with tempfile.TemporaryDirectory() as temp, patch.object(runner, 'TESTS', Path(temp)):
            with self.assertRaisesRegex(ValueError, 'inventory'):
                runner.cases()

    def test_records_failures_and_keeps_running_without_screenshots(self):
        child = MagicMock()
        codes = iter([0, 1, 0])
        def launch(*args, **kwargs):
            self.assertNotIn('KIMO_SHOTS', kwargs['env'])
            self.assertEqual(kwargs['env']['KIMO_SCHEME'], 'dark')
            self.assertTrue(kwargs['start_new_session'])
            kwargs['stdout'].write('PASS assertion\n')
            child.wait.return_value = next(codes)
            return child
        selected = [(name, Path('fixture.py'), [], {'KIMO_SCHEME': 'dark'}, 'test') for name in ['one', 'two', 'three']]
        with tempfile.TemporaryDirectory() as temp, patch.object(runner.subprocess, 'Popen', side_effect=launch), \
             patch.dict(os.environ, {'KIMO_SHOTS': '/unused'}), contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(runner.run(selected, temp, 20))
            results = json.loads((Path(temp) / 'results.json').read_text())
        self.assertEqual([r['exit'] for r in results], [0, 1, 0])

    def test_timeout_stops_only_its_own_process_group(self):
        child = MagicMock(pid=12345)
        child.wait.side_effect = [subprocess.TimeoutExpired('fixture', 1), -9]
        with tempfile.TemporaryDirectory() as temp, patch.object(runner.subprocess, 'Popen', return_value=child), \
             patch.object(runner.os, 'killpg') as kill, contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(runner.run([('slow', Path('fixture.py'), [], {}, 'test')], temp, 1))
            self.assertEqual(json.loads((Path(temp) / 'results.json').read_text())[0]['exit'], 'timeout')
        kill.assert_called_once_with(12345, signal.SIGKILL)
        self.assertEqual(child.wait.call_count, 2)

    def test_launch_error_and_empty_success_log_fail(self):
        for effect, expected in [(OSError('cannot launch'), 'launch-error'), (None, 'empty-log')]:
            child = MagicMock()
            child.wait.return_value = 0
            with tempfile.TemporaryDirectory() as temp, patch.object(runner.subprocess, 'Popen', return_value=child, side_effect=effect), contextlib.redirect_stdout(io.StringIO()):
                self.assertTrue(runner.run([('case', Path('fixture.py'), [], {}, 'test')], temp, 1))
                self.assertEqual(json.loads((Path(temp) / 'results.json').read_text())[0]['exit'], expected)

    def test_success_is_false_failure_flag(self):
        def launch(*args, **kwargs):
            kwargs['stdout'].write('PASS assertion\n')
            return MagicMock(wait=MagicMock(return_value=0))
        with tempfile.TemporaryDirectory() as temp, patch.object(runner.subprocess, 'Popen', side_effect=launch), contextlib.redirect_stdout(io.StringIO()):
            self.assertFalse(runner.run([('case', Path('fixture.py'), [], {}, 'test')], temp, 1))

    def test_invalid_invocations_rejected(self):
        with patch.object(runner.os, 'name', 'nt'), self.assertRaisesRegex(RuntimeError, 'POSIX'):
            runner.run([], 'unused', 1)
        for selected, timeout in [([], 1), ([('case',)], 0)]:
            with self.assertRaisesRegex(ValueError, 'nonempty'):
                runner.run(selected, 'unused', timeout)

    def test_cli_propagates_suite_failure(self):
        with patch.object(sys, 'argv', ['runner', '--output', 'evidence', '--timeout', '12']), \
             patch.object(runner, 'cases', return_value=['fixture']), patch.object(runner, 'run', return_value=True) as run:
            self.assertTrue(runner.main())
        run.assert_called_once_with(['fixture'], 'evidence', 12)
