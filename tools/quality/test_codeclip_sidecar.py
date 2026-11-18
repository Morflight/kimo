"""Sidecar protocol tests with explicit browser doubles; no browser qualification."""
import contextlib
import io
import json
from pathlib import Path
import runpy
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

from test_controls import module, ROOT

sidecar = module('sidecar', 'crates/kimo-core/src/codeclip_sidecar.py')


class Sidecar(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.clip = self.root / 'clip'
        self.clip.mkdir()
        (self.clip / 'index.html').write_text('<canvas/>')
        self.output = self.root / 'frame.rgba'
        self.job = dict(dir=str(self.clip), width=2, height=1,
                        steps=[dict(t=1.25, out=str(self.output))])
        self.page = MagicMock()
        self.page.screenshot.return_value = b'png handled by image double'
        self.events = {}
        self.page.on.side_effect = lambda event, callback: self.events.update({event: callback})
        self.page.evaluate.side_effect = lambda expression, *args: 'function' if expression == 'typeof window.renderFrame' else 'test-gl'
        self.browser = MagicMock()
        self.context = self.browser.new_context.return_value
        self.context.new_page.return_value = self.page
        self.playwright = MagicMock()
        self.playwright.return_value.__enter__.return_value.chromium.launch.return_value = self.browser
        self.img = MagicMock(size=(2, 1))
        self.img.tobytes.return_value = bytes([255, 0, 0, 128, 0, 0, 255, 255])
        self.image = MagicMock()
        self.image.open.return_value.convert.return_value = self.img

    def run_job(self):
        jobfile = self.root / 'job.json'
        jobfile.write_text(json.dumps(self.job))
        output = io.StringIO()
        with patch.dict(sys.modules, {'playwright.sync_api': SimpleNamespace(sync_playwright=self.playwright),
                                      'PIL': SimpleNamespace(Image=self.image)}), \
             patch.object(sys, 'argv', ['sidecar.py', str(jobfile)]), \
             patch.object(sidecar, 'find_chromium', return_value=''), \
             contextlib.redirect_stdout(output):
            sidecar.main()
        return [json.loads(line) for line in output.getvalue().splitlines()]

    def test_rgba_atomic_publication_and_progress(self):
        result = self.run_job()
        self.assertEqual(self.output.read_bytes(), self.img.tobytes.return_value)
        self.assertFalse(Path(str(self.output) + '.part').exists())
        self.assertEqual(result[0]['frame'], 0)
        self.assertEqual(result[0]['t'], 1.25)
        self.assertGreaterEqual(result[0]['ms'], 0)
        self.assertEqual(result[-1], dict(ok=True, frames=1, blocked=[], gl='test-gl'))
        self.page.goto.assert_called_once_with(sidecar.ORIGIN + 'index.html', timeout=60000)
        self.page.screenshot.assert_called_once_with(type='png', omit_background=True, timeout=20000)
        self.context.add_init_script.assert_called_once_with(sidecar.INIT)
        self.browser.close.assert_called_once()
        self.browser.new_context.assert_called_once_with(viewport={'width': 2, 'height': 1}, device_scale_factor=1,
                                                        service_workers='block', java_script_enabled=True)
        render = [c for c in self.page.evaluate.call_args_list if len(c.args) == 2]
        self.assertEqual(render[0].args[1], [1.25, {}, 20000])

    def test_reload_does_not_emit_or_increment_frame(self):
        self.job.update(entry='my page.html', frame_timeout_ms=12, load_timeout_ms=34,
                        chromium=str(self.clip / 'index.html'), lock=str(self.root / 'lock'))
        self.job['steps'] = [{'reload': True}, dict(t=2, params={'speed': 3}, out=str(self.output)),
                             {'reload': True}, dict(t=4, out=str(self.output))]
        with patch.object(sidecar, 'lock_exclusive') as lock:
            result = self.run_job()
        lock.assert_called_once_with(self.job['lock'])
        self.assertEqual([r['frame'] for r in result[:-1]], [0, 1])
        self.assertEqual(result[-1]['frames'], 2)
        self.assertEqual(self.page.goto.call_count, 3)
        self.page.goto.assert_called_with(sidecar.ORIGIN + 'my%20page.html', timeout=34)
        self.assertEqual(self.playwright.return_value.__enter__.return_value.chromium.launch.call_args.kwargs['executable_path'], self.job['chromium'])
        self.assertEqual([c.args[1] for c in self.page.evaluate.call_args_list if len(c.args) == 2],
                         [[2.0, {'speed': 3}, 12], [4.0, {}, 12]])

    def test_route_containment_content_types_and_bounded_diagnostics(self):
        (self.clip / 'a.mjs').write_bytes(b'export {}')
        (self.clip / 'opaque.unknown_kimo').write_bytes(b'raw')
        (self.root / 'secret').write_text('secret')
        (self.clip / 'escape').symlink_to(self.root / 'secret')
        self.run_job()
        handler = self.context.route.call_args.args[1]
        for path, status, content in [('', 200, b'<canvas/>'), ('a.mjs?x=1#f', 200, b'export {}'),
                                      ('opaque.unknown_kimo', 200, b'raw'), ('missing', 404, 'not found'),
                                      ('%2e%2e/secret', 404, 'not found'), ('escape', 404, 'not found')]:
            with self.subTest(path=path):
                route = MagicMock(request=SimpleNamespace(url=sidecar.ORIGIN + path))
                handler(route)
                self.assertEqual(route.fulfill.call_args.kwargs['status'], status)
                self.assertEqual(route.fulfill.call_args.kwargs['body'], content)
                if path.startswith('a.mjs'):
                    self.assertEqual(route.fulfill.call_args.kwargs['headers']['Content-Type'], 'text/javascript')
                route.abort.assert_not_called()
        for _ in range(25):
            route = MagicMock(request=SimpleNamespace(url='https://outside.invalid/' + 'x' * 250))
            handler(route)
            route.abort.assert_called_once()
            route.fulfill.assert_not_called()
        # Inspect the actual mutable diagnostic list captured by the registered route.
        blocked = dict(zip(handler.__code__.co_freevars, [c.cell_contents for c in handler.__closure__]))['blocked']
        self.assertEqual(len(blocked), 20)
        self.assertEqual(len(blocked[-1]), 200)

    def test_load_timeout_and_page_error_context(self):
        for error in [None, 'startup failure']:
            with self.subTest(error=error):
                def timeout(*args, **kwargs):
                    if error:
                        self.events['pageerror'](error)
                    raise TimeoutError('waiting')
                self.page.wait_for_function.side_effect = timeout
                with self.assertRaisesRegex(RuntimeError, 'did not define window.renderFrame.*60000 ms') as caught:
                    self.run_job()
                self.assertEqual('startup failure' in str(caught.exception), bool(error))
                self.assertFalse(self.output.exists())

    def test_ready_requires_callable_renderer(self):
        self.page.evaluate.return_value = 'undefined'
        self.page.evaluate.side_effect = None
        with self.assertRaisesRegex(RuntimeError, 'not a function'):
            self.run_job()
        self.assertFalse(self.output.exists())

    def test_console_errors_abort_but_logs_do_not(self):
        def evaluate(expression, *args):
            if args:
                self.events['console'](SimpleNamespace(type='log', text='informational'))
                self.events['console'](SimpleNamespace(type='error', text='broken frame'))
            return 'function'
        self.page.evaluate.side_effect = evaluate
        with self.assertRaisesRegex(RuntimeError, 'console.error: broken frame'):
            self.run_job()
        self.page.screenshot.assert_not_called()
        self.assertFalse(self.output.exists())

    def test_wrong_dimensions_preserve_previous_frame(self):
        self.output.write_bytes(b'previous')
        self.img.size = (3, 1)
        with self.assertRaisesRegex(RuntimeError, '3x1, expected 2x1'):
            self.run_job()
        self.assertEqual(self.output.read_bytes(), b'previous')

    def test_script_emits_bounded_error_and_exit_three(self):
        self.job['width'] = 'invalid'
        jobfile = self.root / 'job.json'
        jobfile.write_text(json.dumps(self.job))
        with patch.object(sys, 'argv', ['sidecar.py', str(jobfile)]), \
             contextlib.redirect_stdout(io.StringIO()) as output, self.assertRaises(SystemExit) as caught:
            runpy.run_path(str(ROOT / 'crates/kimo-core/src/codeclip_sidecar.py'), run_name='__main__')
        self.assertEqual(caught.exception.code, 3)
        self.assertIn('invalid literal', json.loads(output.getvalue())['error'])

    def test_dependency_error_is_actionable(self):
        jobfile = self.root / 'job.json'
        jobfile.write_text(json.dumps(self.job))
        with patch.dict(sys.modules, {'playwright.sync_api': None}), \
             patch.object(sys, 'argv', ['sidecar.py', str(jobfile)]), \
             contextlib.redirect_stdout(io.StringIO()) as output, self.assertRaises(SystemExit) as caught:
            runpy.run_path(str(ROOT / 'crates/kimo-core/src/codeclip_sidecar.py'), run_name='__main__')
        self.assertEqual(caught.exception.code, 3)
        self.assertIn('pip install playwright pillow', json.loads(output.getvalue())['error'])

    def test_empty_job_reports_zero_frames(self):
        self.job['steps'] = []
        self.assertEqual(self.run_job()[-1]['frames'], 0)
        self.assertFalse(self.output.exists())

    def test_posix_lock_held_by_returned_handle(self):
        with patch('fcntl.flock') as flock:
            handle = sidecar.lock_exclusive(str(self.root / 'lock'))
            try:
                self.assertFalse(handle.closed)
                self.assertEqual(flock.call_args.args[0], handle)
            finally:
                handle.close()

    def test_windows_lock_retries(self):
        windows = SimpleNamespace(LK_LOCK=1, locking=MagicMock(side_effect=[OSError('busy'), None]))
        with patch.object(sidecar.os, 'name', 'nt'), patch.dict(sys.modules, {'msvcrt': windows}), patch.object(sidecar.time, 'sleep') as sleep:
            handle = sidecar.lock_exclusive(str(self.root / 'lock'))
            handle.close()
        self.assertEqual(windows.locking.call_count, 2)
        sleep.assert_called_once_with(0.5)

    def test_browser_discovery_platforms_override_and_fallback(self):
        for platform, name, expected in [('win32', 'nt', 'win'), ('darwin', 'posix', 'mac'), ('linux', 'posix', 'linux')]:
            with self.subTest(platform=platform), patch.object(sidecar.sys, 'platform', platform), \
                 patch.object(sidecar.os, 'name', name), patch.dict(sidecar.os.environ, {'PLAYWRIGHT_BROWSERS_PATH': '/override'}), \
                 patch.object(sidecar.glob, 'glob', return_value=['/candidate-a', '/candidate-z']) as glob, \
                 patch.object(sidecar.os.path, 'isfile', side_effect=lambda path: path.endswith('-a')):
                self.assertEqual(sidecar.find_chromium(), '/candidate-a')
                self.assertIn(expected, glob.call_args.args[0])
                self.assertTrue(glob.call_args.args[0].startswith('/override'))
        with patch.object(sidecar.glob, 'glob', return_value=[]), patch.dict(sidecar.os.environ, {}, clear=True):
            self.assertEqual(sidecar.find_chromium(), '')
