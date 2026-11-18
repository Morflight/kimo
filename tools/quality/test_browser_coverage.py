import hashlib
import importlib.util
import io
import json
from pathlib import Path
import runpy
import sys
import tempfile
import types
import unittest
from unittest.mock import AsyncMock, patch

import browser_coverage as coverage


class CollectorTests(unittest.IsolatedAsyncioTestCase):
    async def test_collects_exact_source_and_preserves_counters(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / 'crates/kimo-gui/ui/app.js'
            source.parent.mkdir(parents=True); source.write_text('function f() { return 1; }')
            script = {'url': 'http://127.0.0.1:456/ui/app.js', 'scriptId': '4', 'source': source.read_text(), 'functions': [{'functionName': 'f', 'ranges': [{'startOffset': 0, 'endOffset': 26, 'count': 3}], 'isBlockCoverage': True}]}
            channel = types.SimpleNamespace(send=AsyncMock(return_value=[{'url': 'https://example.org/ui/x.js'}, {'url': 'http://localhost/other.js'}, script]))
            page = type('Page', (), {})(); page._impl_obj = types.SimpleNamespace(_channel=channel)
            c = coverage.Collector(root / 'raw')
            with patch.object(coverage, 'ROOT', root):
                await c.attach(page); await c.attach(page)
                await c.collect([page]); await c.collect([page])
            self.assertEqual(channel.send.await_count, 2)
            reports = list((root / 'raw').glob('*.json')); self.assertEqual(len(reports), 1)
            data = json.loads(reports[0].read_text())
            self.assertEqual(data, {'result': [{**script, 'url': source.as_uri()}], 'kimoSources': {str(source): hashlib.sha256(source.read_text().encode()).hexdigest()}})
            self.assertFalse(c.sessions)
            coverage.verify_sources(root / 'raw')
            source.write_text('modified after collection')
            with self.assertRaisesRegex(ValueError, 'changed after'):
                coverage.verify_sources(root / 'raw')

    async def test_rejects_wrong_source_and_path_traversal(self):
        for url, message in [('http://localhost/ui/app.js', 'differs'), ('http://localhost/ui/../../outside.js', 'escapes')]:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp); source = root / 'crates/kimo-gui/ui/app.js'
                source.parent.mkdir(parents=True); source.write_text('real source')
                c = coverage.Collector(root / 'raw'); page = object()
                c.sessions[page] = types.SimpleNamespace(send=AsyncMock(return_value=[{'url': url, 'source': 'wrong source'}]))
                with patch.object(coverage, 'ROOT', root), self.assertRaisesRegex(ValueError, message):
                    await c.collect([page])
                self.assertFalse(list((root / 'raw').iterdir()))

    async def test_empty_profile_writes_no_fabricated_report(self):
        with tempfile.TemporaryDirectory() as temp:
            c = coverage.Collector(temp); page = object()
            c.sessions[page] = types.SimpleNamespace(send=AsyncMock(return_value=[]))
            await c.collect([page]); self.assertEqual(list(Path(temp).iterdir()), [])
            with self.assertRaisesRegex(ValueError, 'no browser'):
                coverage.verify_sources(temp)

    async def test_collects_before_each_close_and_for_both_page_creation_apis(self):
        calls = []
        class Page:
            async def close(self, **kwargs):
                calls.append(('page-close', kwargs)); return 1
        class Context:
            pages = []
            async def new_page(self, **kwargs):
                calls.append(('context-new', kwargs)); return page
            async def close(self, **kwargs):
                calls.append(('context-close', kwargs)); return 2
        class Browser:
            contexts = []
            async def new_page(self, **kwargs):
                calls.append(('browser-new', kwargs)); return page
            async def close(self, **kwargs):
                calls.append(('browser-close', kwargs)); return 3
        page, context, browser = Page(), Context(), Browser()
        context.pages = [page]; browser.contexts = [context]
        async def collect(pages): calls.append(('collect', list(pages)))
        c = types.SimpleNamespace(attach=AsyncMock(), collect=collect)
        coverage.install(c, Browser, Context, Page)
        self.assertIs(await context.new_page(a=1), page)
        self.assertIs(await browser.new_page(b=2), page)
        self.assertEqual(c.attach.await_count, 2)
        self.assertEqual(await page.close(reason='done'), 1)
        self.assertEqual(await context.close(reason='done'), 2)
        self.assertEqual(await browser.close(reason='done'), 3)
        self.assertEqual(calls, [('context-new', {'a': 1}), ('browser-new', {'b': 2}),
                                ('collect', [page]), ('page-close', {'reason': 'done'}),
                                ('collect', [page]), ('context-close', {'reason': 'done'}),
                                ('collect', [page]), ('browser-close', {'reason': 'done'})])


class EntrypointTests(unittest.TestCase):
    def test_script_arguments_and_main_entrypoint(self):
        fake = types.ModuleType('playwright.async_api')
        fake.Browser, fake.BrowserContext, fake.Page = object(), object(), object()
        with tempfile.TemporaryDirectory() as temp:
            for direct in [True, False]:
                argv = ['collector', '--output', temp, 'fixture.py', '--some-flag', 'value']
                with patch.dict(sys.modules, {'playwright': types.ModuleType('playwright'), 'playwright.async_api': fake}), \
                     patch.object(sys, 'argv', argv), patch.object(sys, 'path', list(sys.path)), \
                     patch.object(coverage, 'install') as install, patch('runpy.run_path') as run:
                    if direct:
                        coverage.main(); install.assert_called_once()
                    else:
                        # Execute the CLI guard too, with the real install accepting dummy types.
                        fake.Browser = type('B', (), {'close': None, 'new_page': None})
                        fake.BrowserContext = type('C', (), {'close': None, 'new_page': None})
                        fake.Page = type('P', (), {'close': None})
                        exec(compile(Path(coverage.__file__).read_text(), coverage.__file__, 'exec'), {'__name__': '__main__', '__file__': coverage.__file__})
                    run.assert_called_once_with('fixture.py', run_name='__main__')
                    self.assertEqual(sys.argv, ['fixture.py', '--some-flag', 'value'])
                    self.assertEqual(sys.path[0], str(Path('fixture.py').resolve().parent))
