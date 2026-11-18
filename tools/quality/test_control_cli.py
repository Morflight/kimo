"""CLI exit statuses and diagnostics are part of the quality gate contract."""
import contextlib
import io
import json
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch, MagicMock
from test_controls import ROOT, rustcov


class ControlCli(unittest.TestCase):
    def invoke(self, script, args, expected):
        with patch.object(sys, 'argv', [script, *args]), contextlib.redirect_stdout(io.StringIO()) as output:
            with self.assertRaises(SystemExit) as caught:
                runpy.run_path(str(ROOT / script), run_name='__main__')
        self.assertEqual(caught.exception.code, expected, output.getvalue())
        return output.getvalue()

    def test_log_cli_writes_sorted_failures_and_rejects_empty(self):
        with tempfile.TemporaryDirectory() as temp:
            log, output = Path(temp) / 'log', Path(temp) / 'failures'
            for body, status, failures in [('', 1, ''),
                ('test z ... FAILED\ntest a ... FAILED\ntest result: FAILED. 1 passed; 2 failed; 0 ignored', 1, 'a\nz\n'),
                ('test result: ok. 2 passed; 0 failed; 0 ignored', 0, '')]:
                log.write_text(body)
                text = self.invoke('tools/ci/compare_failures.py', [str(log), 'unused', str(output)], status)
                self.assertEqual(output.read_text(), failures)
                self.assertIn('FAIL:' if status else 'Completed tests passed', text)

    def test_rust_cli_reports_missing_then_complete_metrics(self):
        name = 'crates/kimo-core/src/example.rs'
        with tempfile.TemporaryDirectory() as temp, patch('subprocess.check_output', return_value=name):
            report = Path(temp) / 'report.json'
            for summary, status in [({}, 1), ({m: {'count': 1, 'covered': 1} for m in ['lines', 'functions', 'regions']}, 0)]:
                report.write_text(json.dumps({'data': [{'files': [{'filename': name, 'summary': summary},
                                                                {'filename': 'external.rs', 'summary': {}}]}]}))
                text = self.invoke('tools/quality/check_rust_coverage.py', ['--prefix', 'crates/kimo-core/', str(report)], status)
                self.assertEqual('FAIL:' in text, bool(status))
        with tempfile.TemporaryDirectory() as temp, patch('subprocess.check_output', return_value=''):
            report = Path(temp) / 'report.json'
            report.write_text('{"data": []}')
            self.assertEqual(rustcov.check([report], 'crates/'), ['empty production inventory'])

    def test_contract_cli_checks_actual_catalog_and_rejects_absent_inventory(self):
        text = self.invoke('tools/quality/check_contracts.py', [], 0)
        self.assertIn('0 error(s)', text)
        with patch('subprocess.check_output', return_value=''):
            text = self.invoke('tools/quality/check_contracts.py', [], 1)
        self.assertIn('stale/non-production source', text)

    def test_python_cli_requires_exact_counts(self):
        name = 'crates/core/src/helper.py'
        with tempfile.TemporaryDirectory() as temp, patch('subprocess.check_output', return_value=name):
            report = Path(temp) / 'report.json'
            for summary, status in [({}, 1), (dict(num_statements=1, covered_lines=1, missing_lines=0,
                                                   num_branches=0, covered_branches=0, missing_branches=0), 0)]:
                report.write_text(json.dumps({'meta': {'branch_coverage': True}, 'files': {name: {'summary': summary}}}))
                text = self.invoke('tools/quality/check_python_coverage.py', [str(report)], status)
                self.assertEqual('FAIL:' in text, bool(status))

    def test_controls_direct_entry_invokes_unittest(self):
        with patch('unittest.main') as main:
            runpy.run_path(str(ROOT / 'tools/quality/test_controls.py'), run_name='__main__')
        main.assert_called_once_with()

    def test_browser_cli_executes_full_inventory_and_writes_receipts(self):
        def launch(*args, **kwargs):
            kwargs['stdout'].write('PASS synthetic runner fixture\n')
            return MagicMock(wait=MagicMock(return_value=0))
        with tempfile.TemporaryDirectory() as temp, patch('subprocess.Popen', side_effect=launch) as launched:
            self.invoke('tools/quality/run_browser_tests.py', ['--output', temp], 0)
            results = json.loads((Path(temp) / 'results.json').read_text())
            self.assertGreater(len(results), 1)
            self.assertEqual(len(results), launched.call_count)
            self.assertTrue(all(row['exit'] == 0 for row in results))
