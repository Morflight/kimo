"""Collect real Chromium V8 counters for a Python browser proof, before pages close.

Run separately from timing acceptance: profiler instrumentation changes timings.
Only URL-to-local-source mapping is performed; counter values are never rewritten.
"""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import sys
from urllib.parse import unquote, urlparse
from urllib.request import url2pathname
import uuid

ROOT = Path(__file__).resolve().parents[2]


class Collector:
    def __init__(self, output):
        self.output = Path(output)
        self.output.mkdir(parents=True, exist_ok=True)
        self.sessions = {}

    async def attach(self, page):
        if page in self.sessions:
            return
        # The pinned Playwright backend exposes its Chromium coverage protocol
        # even though the Python client has no public page.coverage facade.
        channel = page._impl_obj._channel
        await channel.send('startJSCoverage', None, {'resetOnNavigation': False, 'reportAnonymousScripts': False})
        self.sessions[page] = channel

    async def collect(self, pages):
        for page in list(pages):
            channel = self.sessions.pop(page, None)
            if channel is None:
                continue
            result = await channel.send('stopJSCoverage', None, {})
            scripts, sources = [], {}
            for script in result:
                url = urlparse(script['url'])
                if url.hostname not in ('127.0.0.1', 'localhost') or not url.path.startswith('/ui/') or not url.path.endswith('.js'):
                    continue
                path = (ROOT / 'crates/kimo-gui' / unquote(url.path).lstrip('/')).resolve()
                if not path.is_relative_to((ROOT / 'crates/kimo-gui/ui').resolve()):
                    raise ValueError('coverage URL escapes UI source directory')
                source = script.get('source')
                if source != path.read_text():
                    raise ValueError(f'coverage source differs from checkout: {path}')
                # c8 indexes executed sources only for file URLs (including --all).
                scripts.append({**script, 'url': path.as_uri()})
                sources[str(path)] = hashlib.sha256(source.encode()).hexdigest()
            if scripts:
                data = {'result': scripts, 'kimoSources': sources}
                (self.output / f'coverage-browser-{uuid.uuid4().hex}.json').write_text(json.dumps(data))


def verify_sources(output):
    reports = list(Path(output).glob('coverage-browser-*.json'))
    if not reports:
        raise ValueError('no browser coverage reports')
    for report in reports:
        data = json.loads(report.read_text())
        hashes = data['kimoSources']
        for script in data['result']:
            path = Path(url2pathname(urlparse(script['url']).path))
            if hashlib.sha256(path.read_text().encode()).hexdigest() != hashes[str(path)]:
                raise ValueError(f'coverage source changed after collection: {path}')


def install(collector, browser_type, context_type, page_type):
    original_page = page_type.close
    original_context = context_type.close
    original_browser = browser_type.close
    original_new_context_page = context_type.new_page
    original_new_browser_page = browser_type.new_page

    async def context_page(self, *args, **kwargs):
        page = await original_new_context_page(self, *args, **kwargs)
        await collector.attach(page)
        return page

    async def browser_page(self, *args, **kwargs):
        page = await original_new_browser_page(self, *args, **kwargs)
        await collector.attach(page)
        return page

    async def close_page(self, *args, **kwargs):
        await collector.collect([self])
        return await original_page(self, *args, **kwargs)

    async def close_context(self, *args, **kwargs):
        await collector.collect(self.pages)
        return await original_context(self, *args, **kwargs)

    async def close_browser(self, *args, **kwargs):
        await collector.collect([p for context in self.contexts for p in context.pages])
        return await original_browser(self, *args, **kwargs)

    context_type.new_page = context_page
    browser_type.new_page = browser_page
    page_type.close = close_page
    context_type.close = close_context
    browser_type.close = close_browser


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('script')
    parser.add_argument('args', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    from playwright.async_api import Browser, BrowserContext, Page
    install(Collector(args.output), Browser, BrowserContext, Page)
    sys.argv = [args.script, *args.args]
    sys.path.insert(0, str(Path(args.script).resolve().parent))
    runpy.run_path(args.script, run_name='__main__')


if __name__ == '__main__':
    main()
