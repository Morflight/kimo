"""The Windows response-file fallback retains every coverage object and flag."""
import contextlib
import io
from pathlib import Path
import runpy
import shlex
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import rust_coverage_report as report


class RustReport(unittest.TestCase):
    def test_only_windows_export_length_failures_are_retried(self):
        prefix='could not execute process `'
        suffix='` (never executed): filename too long (os error 206)'
        for error in ['other failure', 'os error 206', prefix+"'broken"+suffix,
                      prefix+"'llvm-cov.exe'"+suffix, prefix+"'other.exe' export"+suffix,
                      prefix+"'llvm-cov.exe' show"+suffix]:
            self.assertIsNone(report.long_command(error),error)
        command=[r'C:\Tool chain\llvm-cov.exe','export','-object',r'D:\build\one.exe','-object',r'D:\build\two.exe','-ignore-filename-regex',r'\rustc\source']
        self.assertEqual(report.long_command(prefix+shlex.join(command)+suffix),command)
        self.assertEqual(shlex.split(report.response_text(command,False)),command)
        self.assertEqual(report.response_text(['export','-object',r'D:\build\one.exe'],True),r'export -object D:\build\one.exe')
        self.assertEqual(report.response_text(['-object','D:/space dir/one.exe'],True),'-object "D:/space dir/one.exe"')

    def test_regular_success_and_failure_keep_status_and_diagnostics(self):
        for status in [0,7]:
            result=subprocess.CompletedProcess([],status,'output\n','diagnostic\n')
            with patch.object(report.subprocess,'run',return_value=result) as run, contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
                self.assertEqual(report.report('result.json',['--manifest-path','desktop/Cargo.toml']),status)
            self.assertEqual(out.getvalue(),'output\n');self.assertEqual(err.getvalue(),'diagnostic\n')
            self.assertEqual(run.call_args.args[0],['cargo','llvm-cov','report','--json','--output-path','result.json','--manifest-path','desktop/Cargo.toml'])
            self.assertEqual(run.call_count,1)

    def test_response_file_retains_large_inventory_writes_report_and_cleans_up(self):
        command=['llvm-cov.exe','export','-format=text','-instr-profile=profile data']
        for index in range(500):command.extend(['-object',f'build dir/test-{index}.exe'])
        command.extend(['-ignore-filename-regex','original-filter'])
        error='could not execute process `'+shlex.join(command)+'` (never executed): filename too long (os error 206)'
        for status in [0,9]:
            responses=[]
            def invoke(argv,**kwargs):
                if argv[0]=='cargo':return subprocess.CompletedProcess(argv,1,'',error)
                self.assertEqual(argv[0],'llvm-cov.exe');self.assertEqual(len(argv),2)
                path=Path(argv[1][1:]);responses.append(path)
                self.assertEqual(shlex.split(path.read_text()),command[1:])
                kwargs['stdout'].write(b'{"data":[],"fixture":"all objects retained"}')
                return subprocess.CompletedProcess(argv,status)
            with tempfile.TemporaryDirectory() as directory, patch.object(report.subprocess,'run',side_effect=invoke), contextlib.redirect_stderr(io.StringIO()) as err:
                output=Path(directory)/'report.json';self.assertEqual(report.report(output,[]),status)
                self.assertIn('all objects retained',output.read_text());self.assertIn('all original arguments',err.getvalue())
            self.assertFalse(responses[0].exists())

    def test_cli_passes_manifest_and_report_destination(self):
        with patch.object(sys,'argv',['report','--output-path','fixture.json','--manifest-path','desktop/Cargo.toml']),patch('subprocess.run',return_value=subprocess.CompletedProcess([],5,'','fixture refusal')),contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as caught:
                runpy.run_path(str(Path(report.__file__)),run_name='__main__')
        self.assertEqual(caught.exception.code,5)
