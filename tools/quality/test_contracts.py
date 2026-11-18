"""Check coherence with a real isolated Git index, not the maintainer's checkout."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from test_controls import contracts


class Contracts(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.addCleanup(patch.stopall)
        patch.object(contracts, 'ROOT', self.root).start()
        self.git('init', '-q')
        self.source = 'crates/core/src/main.rs'
        self.docs = 'docs/feature.md'
        self.tests = 'tests/feature.py'
        for file in [self.source, self.docs, self.tests]:
            path = self.root / file
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('initial')
        self.entry = dict(id='feature', docs=[self.docs], tests=[self.tests], sources=[self.source])
        self.catalog = self.root / 'docs/features/catalog.json'
        self.catalog.parent.mkdir()
        self.write_catalog()
        self.git('add', '.')
        self.git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-qm', 'fixture')

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, text=True)

    def write_catalog(self, entries=None):
        self.catalog.write_text(json.dumps({'features': [self.entry] if entries is None else entries}))

    def test_unchanged_and_docs_only_pass(self):
        self.assertEqual(contracts.check(), [])
        (self.root / self.docs).write_text('updated')
        self.assertEqual(contracts.check('HEAD'), [])

    def test_source_requires_both_docs_and_tests(self):
        (self.root / self.source).write_text('changed')
        self.assertEqual(len(contracts.check('HEAD')), 2)
        (self.root / self.docs).write_text('updated')
        self.assertIn('behavioral tests', contracts.check('HEAD')[0])
        (self.root / self.tests).write_text('updated')
        self.assertEqual(contracts.check('HEAD'), [])

    def test_missing_duplicate_unclassified_and_stale(self):
        self.write_catalog([self.entry, self.entry])
        self.assertIn('duplicate owner', contracts.check()[0])
        self.write_catalog([])
        self.assertIn('unclassified production file', contracts.check()[0])
        self.entry['sources'].append(self.docs)
        self.write_catalog()
        self.assertTrue(any('stale/non-production' in e for e in contracts.check()))
        (self.root / self.tests).unlink()
        self.assertTrue(any('missing tests/feature.py' in e for e in contracts.check()))

    def test_empty_links_rejected(self):
        for key in ['docs', 'tests', 'sources']:
            entry = dict(self.entry, **{key: []})
            self.write_catalog([entry])
            self.assertIn('must all be nonempty', contracts.check()[0])
