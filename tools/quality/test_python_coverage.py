import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from test_controls import module

pythoncov = module('pythoncov', 'tools/quality/check_python_coverage.py')


class PythonCoverage(unittest.TestCase):
    source = 'crates/kimo-core/src/color/ocio_bridge.py'

    def check(self, summary=None, files=None, branch=True, inventory=None):
        if summary is None:
            summary = dict(num_statements=5, covered_lines=5, missing_lines=0,
                           num_branches=2, covered_branches=2, missing_branches=0)
        if files is None:
            files = {self.source: {'summary': summary}}
        with tempfile.TemporaryDirectory() as temp:
            report = Path(temp) / 'coverage.json'
            report.write_text(json.dumps(dict(meta={'branch_coverage': branch}, files=files)))
            with patch.object(pythoncov.subprocess, 'check_output', return_value=self.source if inventory is None else inventory):
                return pythoncov.check(report)

    def test_nested_runtime_is_required_even_when_not_imported(self):
        self.assertEqual(self.check(), [])
        self.assertIn('absent from coverage report', self.check(files={})[0])

    def test_no_rounded_or_aggregate_pass(self):
        summary = dict(num_statements=10000, covered_lines=9999, missing_lines=1,
                       num_branches=2, covered_branches=1, missing_branches=1)
        self.assertEqual(len(self.check(summary=summary)), 2)

    def test_missing_invalid_negative_and_boolean_counts_fail(self):
        for value in [None, -1, True, '5']:
            with self.subTest(value=value):
                self.assertTrue(self.check(summary={'num_statements': value}))

    def test_absent_branch_measurement_and_empty_inventory_fail(self):
        self.assertIn('branch coverage was not collected', self.check(branch=False))
        self.assertIn('empty Python inventory', self.check(inventory=''))

    def test_windows_separators(self):
        summary = dict(num_statements=1, covered_lines=1, missing_lines=0,
                       num_branches=0, covered_branches=0, missing_branches=0)
        self.assertEqual(self.check(files={self.source.replace('/', '\\'): {'summary': summary}}), [])

    def test_scope_keeps_tools_and_nested_runtime(self):
        for path in [self.source, 'tools/ci/compare_failures.py', 'tools/quality/test_controls.py']:
            self.assertTrue(pythoncov.in_scope(path))
        for path in ['README.md', 'tests/native_proof.py', 'crates/a/src/vendor/x.py', 'crates/a/tests/x.py']:
            self.assertFalse(pythoncov.in_scope(path))
