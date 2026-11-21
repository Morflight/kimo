import contextlib
import io
import json
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch
from test_controls import ROOT, module

strategy = module('strategy', 'tools/quality/check_test_strategy.py')

HEAVY = {'binary': 'kimo-core::plates', 'test': 'rolling_shutter_plate', 'category': 'accuracy',
         'fast_test': 'kimo-core::stabilize_tiny::rolling_shutter_keys_on_tiny_media'}


class Repo:
    """A synthetic checkout: register, nextest config and one Rust source file."""

    def __init__(self, temp, tests, fast=None, heavy='[profile.heavy]\nfail-fast = false\n'):
        self.root = Path(temp)
        (self.root / '.config').mkdir()
        (self.root / 'crates/kimo-core/tests').mkdir(parents=True)
        (self.root / 'crates/kimo-core/tests/stabilize_tiny.rs').write_text(
            '#[test]\nfn rolling_shutter_keys_on_tiny_media() {}\nfn rolling_shutter_plate () {}\n')
        target = self.root / 'crates/kimo-core/target'
        target.mkdir()
        (target / 'stale.rs').write_text('fn only_in_build_output() {}\n')
        (self.root / '.config/heavy-tests.json').write_text(json.dumps({'tests': tests}))
        if fast is None:
            fast = strategy.expected_filter(tests)
        (self.root / '.config/nextest.toml').write_text(
            f'[profile.fast]\ndefault-filter = """\n{fast}"""\n' + heavy)


class TestStrategy(unittest.TestCase):
    def check(self, tests, **kwargs):
        with tempfile.TemporaryDirectory() as temp:
            return strategy.check(Repo(temp, tests, **kwargs).root)

    def test_triaged_register_matching_the_fast_filter_passes(self):
        self.assertEqual(self.check([HEAVY]), [])

    def test_the_actual_repository_follows_the_strategy(self):
        self.assertEqual(strategy.check(), [])

    def test_fast_filter_must_exclude_exactly_the_register(self):
        errors = self.check([HEAVY], fast='not (\n)\n')
        self.assertEqual(len(errors), 1)
        self.assertIn('rolling_shutter_plate', errors[0])

    def test_heavy_profile_must_exist_and_run_everything(self):
        self.assertIn('[profile.heavy]', self.check([HEAVY], heavy='')[0])
        filtered = '[profile.heavy]\ndefault-filter = "all()"\n'
        self.assertIn('without a default-filter', self.check([HEAVY], heavy=filtered)[0])

    def test_triaged_heavy_test_needs_an_existing_distinct_fast_counterpart(self):
        for fast, message in [(None, 'fast_test must name'), ('', 'fast_test must name'),
                              ('kimo-core::stabilize_tiny::missing', 'is not defined'),
                              ('kimo-core::only_in_build_output', 'is not defined'),
                              ('kimo-core::plates::rolling_shutter_plate', 'itself registered as heavy')]:
            with self.subTest(fast=fast):
                errors = self.check([dict(HEAVY, fast_test=fast)])
                self.assertEqual(len(errors), 1)
                self.assertIn(message, errors[0])

    def test_categories_are_closed(self):
        self.assertIn('category must be one of', self.check([dict(HEAVY, category='slow')])[0])
        self.assertEqual(self.check([dict(HEAVY, category='untriaged', fast_test=None)]), [])

    def test_untriaged_debt_cannot_grow(self):
        legacy = [{'binary': 'kimo-core::legacy', 'test': f't{i:03}', 'category': 'untriaged'}
                  for i in range(strategy.LEGACY_UNTRIAGED_MAX + 1)]
        errors = self.check(legacy)
        self.assertEqual(len(errors), 1)
        self.assertIn('exceed the legacy baseline', errors[0])
        self.assertEqual(self.check(legacy[:-1]), [])

    def test_malformed_duplicate_and_unsorted_entries_fail(self):
        errors = self.check([{'binary': 'kimo-core', 'test': ''}, HEAVY, HEAVY])
        self.assertTrue(any('entry 0: binary and test are required' in e for e in errors))
        self.assertTrue(any('duplicate entry' in e for e in errors))
        later = dict(HEAVY, binary='kimo-core::z')
        self.assertIn('must be sorted', self.check([later, HEAVY])[0])

    def test_register_must_be_a_list_of_objects(self):
        for register in [[], {'tests': 'x'}, {'tests': ['x']}]:
            with self.subTest(register=register), tempfile.TemporaryDirectory() as temp:
                repo = Repo(temp, [])
                (repo.root / '.config/heavy-tests.json').write_text(json.dumps(register))
                self.assertEqual(strategy.check(repo.root), ['.config/heavy-tests.json: "tests" must be a list of objects'])

    def test_cli_reports_errors_and_exit_status(self):
        for errors, status, text in [([], 0, '0 error(s)'), (['broken'], 1, 'FAIL: broken')]:
            with self.subTest(errors=errors), patch.object(sys, 'argv', ['check_test_strategy.py']), \
                    contextlib.redirect_stdout(io.StringIO()) as output:
                with patch.object(strategy, 'check', return_value=errors):
                    self.assertEqual(strategy.main(), bool(status))
                self.assertIn(text, output.getvalue())

    def test_direct_entry_exits_with_the_check_status(self):
        with contextlib.redirect_stdout(io.StringIO()) as output, self.assertRaises(SystemExit) as caught:
            runpy.run_path(str(ROOT / 'tools/quality/check_test_strategy.py'), run_name='__main__')
        self.assertEqual(caught.exception.code, False)
        self.assertIn('0 error(s)', output.getvalue())
