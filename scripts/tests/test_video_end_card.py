"""Real FFmpeg/Pillow regression in the prepared, network-disabled Skill image."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('finish_video', '/skill/scripts/finish_video.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class EndCardTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(dir='/work')
        self.previous = Path.cwd()
        os.chdir(self.directory.name)

    def tearDown(self):
        os.chdir(self.previous)
        self.directory.cleanup()

    def source(self, audio=True):
        cmd = ['ffmpeg', '-v', 'error', '-y', '-threads', '2', '-filter_threads', '1', '-filter_complex_threads', '1', '-f', 'lavfi', '-i', 'color=c=blue:s=1344x768:r=24:d=2']
        if audio:
            cmd += ['-f', 'lavfi', '-i', 'sine=frequency=440:duration=2', '-c:a', 'aac']
        subprocess.run(cmd + ['-c:v', 'libx264', '-threads', '2', '-pix_fmt', 'yuv420p', '-t', '2', 'source.mp4'], check=True)

    def test_real_video_exact_end_card_keeps_duration_audio_and_last_frame(self):
        self.source()
        url = 'https://github.com/jason571/Exoharness'
        data = {'video_file': 'source.mp4', 'duration_seconds': 2, 'end_card_seconds': .75, 'lines': ['ExoHarness', url]}
        result = module.finish(data, 'final.mp4')
        self.assertAlmostEqual(result['duration_seconds'], 2, delta=.05)
        self.assertTrue(result['audio_present'])
        self.assertEqual(result['exact_end_card_lines'][1], url)
        subprocess.run(['ffmpeg', '-v', 'error', '-threads', '2', '-filter_threads', '1', '-i', 'final.mp4', '-f', 'null', '-'], check=True)
        subprocess.run(['ffmpeg', '-v', 'error', '-threads', '2', '-filter_threads', '1', '-sseof', '-0.1', '-i', 'final.mp4', '-frames:v', '1', 'last.png'], check=True)
        from PIL import Image, ImageChops, ImageStat
        actual, card = Image.open('last.png').convert('RGB'), Image.open('final.end-card.png').convert('RGB')
        self.assertLess(sum(ImageStat.Stat(ImageChops.difference(actual, card)).mean) / 3, 8)

    def test_silent_source_stays_silent_and_other_aspect_ratios_render(self):
        self.source(audio=False)
        result = module.finish({'video_file': 'source.mp4', 'end_card_seconds': .5, 'lines': ['AI', '准确网址 https://example.test/a?x=1&y=2']}, 'final.mp4')
        self.assertFalse(result['audio_present'])
        module.render_card(360, 640, ['ExoHarness', 'https://github.com/jason571/Exoharness'], Path('portrait.png'))
        self.assertTrue(Path('portrait.png').is_file())

    def test_unsafe_paths_bad_dimensions_and_overflow_never_fetch_or_truncate(self):
        for path in ['https://example.test/video.mp4', '../video.mp4', '/etc/passwd']:
            with self.subTest(path=path), self.assertRaises(ValueError):
                module.local_file(path, existing=True)
        for width, lines in [(8192, ['title']), (640, []), (640, ['x\nnext']), (160, ['W' * 512])]:
            with self.subTest(width=width, lines=len(lines)), self.assertRaises(ValueError):
                module.render_card(width, 360, lines, Path('invalid.png'))
        self.assertFalse(Path('invalid.png').exists())

    def test_cannot_overwrite_source_extend_video_or_use_nonfinite_duration(self):
        self.source(audio=False)
        original = Path('source.mp4').read_bytes()
        for data, output in [({'video_file': 'source.mp4', 'lines': ['A']}, 'source.mp4'),
                ({'video_file': 'source.mp4', 'duration_seconds': 3, 'lines': ['A']}, 'new.mp4'),
                ({'video_file': 'source.mp4', 'duration_seconds': float('nan'), 'lines': ['A']}, 'new.mp4'),
                ({'video_file': 'source.mp4', 'end_card_seconds': 0, 'lines': ['A']}, 'new.mp4')]:
            with self.subTest(data=data), self.assertRaises(ValueError):
                module.finish(data, output)
        self.assertEqual(Path('source.mp4').read_bytes(), original)
        self.assertFalse(Path('new.mp4').exists())


if __name__ == '__main__':
    unittest.main()
