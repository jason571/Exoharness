"""Run in the existing Skill OCI image with /skill and writable /work mounts."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, '/skill/scripts')
from render_deck import ContentError, render


class PaletteTests(unittest.TestCase):
    def test_invalid_palette_diagnostic_identifies_exact_contract_and_retains_content(self):
        for palette, message in [({'primary': '142D40'}, 'unsupported key'),
                                 ({'ink': '#142D40'}, 'without #'),
                                 ({'accent': ['007F86']}, 'without #')]:
            with self.subTest(palette=palette), tempfile.TemporaryDirectory(dir='/work') as directory:
                output = Path(directory) / 'report.pptx'
                data = {'title': 'Exact saved input', 'palette': palette,
                        'slides': [{'layout': 'cover', 'title': '中国经济', 'subtitle': '已确认的统计期'}]}
                with self.assertRaisesRegex(ContentError, message):
                    render(data, output)
                self.assertEqual(json.loads(output.with_name('.report.pptx.content.json').read_text()), data)
                self.assertFalse(output.exists())

    def test_default_and_supported_palette_render_real_pptx(self):
        for palette in [None, {'ink': '142D40', 'accent': '007f86'}]:
            with self.subTest(palette=palette), tempfile.TemporaryDirectory(dir='/work') as directory:
                output = Path(directory) / 'report.pptx'
                data = {'slides': [{'layout': 'cover', 'title': '中国经济', 'subtitle': '已确认的统计期'}]}
                if palette is not None:
                    data['palette'] = palette
                result = render(data, output)
                self.assertTrue(output.is_file())
                self.assertEqual(result['slides'], 1)


if __name__ == '__main__':
    unittest.main()
