"""Binary streaming contract; real OCIO color accuracy remains a native test."""
import array
import io
import runpy
import struct
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch
from test_controls import ROOT

SCRIPT = str(ROOT / 'crates/kimo-core/src/color/ocio_bridge.py')


class ShortReads(io.BytesIO):
    def read(self, size=-1):
        return super().read(min(size, 7))


class OcioBridge(unittest.TestCase):
    def run_bridge(self, data=b'', mode='sdr', version='2.5.1', missing=False, byteorder='little'):
        self.output = io.BytesIO()
        self.cpu = MagicMock()
        self.ocio = MagicMock()
        self.ocio.GetVersion.return_value = version
        self.ocio.Config.CreateFromBuiltinConfig.return_value.getProcessor.return_value.getDefaultCPUProcessor.return_value = self.cpu
        self.ocio.PackedImageDesc.side_effect = lambda a, w, h, channels: (a, w, h, channels)
        # Deliberate transformation proves that processed bytes, rather than input, are emitted.
        def transform(desc):
            pixels, w, h, channels = desc
            self.assertEqual((w, h, channels), (1, 1, 4))
            pixels[0] = 0.75
        self.cpu.apply.side_effect = transform
        with patch.dict(sys.modules, {'PyOpenColorIO': None if missing else self.ocio}), \
             patch.object(sys, 'argv', ['ocio_bridge.py', '1', '1', mode]), \
             patch.object(sys, 'stdin', SimpleNamespace(buffer=ShortReads(data))), \
             patch.object(sys, 'stdout', SimpleNamespace(buffer=self.output)), \
             patch.object(sys, 'byteorder', byteorder):
            runpy.run_path(SCRIPT, run_name='__main__')
        return self.output.getvalue()

    def test_multiple_frames_short_reads_and_display_views(self):
        frame = struct.pack('<4f', 0.1, 0.2, 0.3, 1)
        expected = struct.pack('<4f', 0.75, 0.2, 0.3, 1)
        for mode, display, view in [('sdr', 'sRGB - Display', 'ACES 2.0 - SDR 100 nits (Rec.709)'),
                                     ('pq2020', 'Rec.2100-PQ - Display', 'ACES 2.0 - HDR 1000 nits (Rec.2020)')]:
            with self.subTest(mode=mode):
                self.assertEqual(self.run_bridge(frame * 2, mode), expected * 2)
                self.assertEqual(self.cpu.apply.call_count, 2)
                self.ocio.DisplayViewTransform.assert_called_once_with(src='ACEScg', display=display, view=view)
                self.ocio.Config.CreateFromBuiltinConfig.assert_called_once_with('cg-config-v4.0.0_aces-v2.0_ocio-v2.5')

    def test_big_endian_host_conversion(self):
        # Emulate native big-endian array storage on the little-endian test host.
        class BigEndianArray:
            def frombytes(self, data):
                self.data = bytearray(data)
            def byteswap(self):
                self.data = bytearray(b''.join(self.data[i:i+4][::-1] for i in range(0, len(self.data), 4)))
            def __setitem__(self, index, value):
                self.data[index*4:index*4+4] = struct.pack('>f', value)
            def tobytes(self):
                return bytes(self.data)
        with patch.object(array, 'array', return_value=BigEndianArray()):
            output = self.run_bridge(struct.pack('<4f', 0.1, 0.2, 0.3, 1), byteorder='big')
        self.assertEqual(output, struct.pack('<4f', 0.75, 0.2, 0.3, 1))

    def test_empty_input_produces_no_frame(self):
        self.assertEqual(self.run_bridge(), b'')
        self.cpu.apply.assert_not_called()

    def test_truncated_frame_is_not_emitted(self):
        with self.assertRaisesRegex(SystemExit, 'truncated input frame'):
            self.run_bridge(b'x' * 15)
        self.assertEqual(self.output.getvalue(), b'')
        self.cpu.apply.assert_not_called()

    def test_missing_dependency_version_and_mode_are_rejected(self):
        for kwargs, error in [({'missing': True}, 'PyOpenColorIO missing'),
                              ({'version': '2.5.2'}, '2.5.1 exactly'),
                              ({'mode': 'unknown'}, 'unsupported reference view')]:
            with self.subTest(kwargs=kwargs), self.assertRaisesRegex(SystemExit, error):
                self.run_bridge(**kwargs)
            self.assertEqual(self.output.getvalue(), b'')
