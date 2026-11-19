import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import json
import tempfile

ROOT = Path(__file__).resolve().parents[2]
def module(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m

contracts = module('contracts', 'tools/quality/check_contracts.py')
rustcov = module('rustcov', 'tools/quality/check_rust_coverage.py')

class Controls(unittest.TestCase):
    def test_runtime_inventory(self):
        self.assertTrue(contracts.production('crates/kimo-core/src/codeclip_sidecar.py'))
        self.assertTrue(contracts.production('crates/kimo-gui/ui/playback.js'))
        self.assertFalse(contracts.production('crates/kimo-gui/ui/vendor/three.js'))
        self.assertFalse(contracts.production('crates/kimo-core/tests/preview_stream.rs'))

class RustCoverage(unittest.TestCase):
    def check_report(self, files):
        with tempfile.TemporaryDirectory() as temp:
            report = Path(temp) / 'coverage.json'
            report.write_text(json.dumps({'data': [{'files': files}]}))
            with patch.object(rustcov.subprocess, 'check_output', return_value='crates/kimo-core/src/a.rs\n'):
                return rustcov.check([report], 'crates/kimo-core/')
    def test_absent_file_fails(self):
        self.assertTrue(self.check_report([]))
    def test_uncovered_counts_fail_without_rounding(self):
        summary = {m: {'count': 10000, 'covered': 9999} for m in ['lines', 'functions', 'regions', 'branches']}
        self.assertTrue(self.check_report([{'filename': '/checkout/crates/kimo-core/src/a.rs', 'summary': summary}]))
    def test_complete_file_passes(self):
        summary = {m: {'count': 2, 'covered': 2} for m in ['lines', 'functions', 'regions', 'branches']}
        self.assertEqual(self.check_report([{'filename': 'C:/repo/crates/kimo-core/src/a.rs', 'summary': summary}]), [])
    def test_missing_metrics_fail(self):
        self.assertTrue(self.check_report([{'filename': '/repo/crates/kimo-core/src/a.rs', 'summary': {}}]))

if __name__ == '__main__':
    unittest.main()
