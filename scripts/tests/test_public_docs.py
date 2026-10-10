"""Verify repository documentation links and documented local build artifacts."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class PublicDocumentationTests(unittest.TestCase):
    def test_relative_markdown_links_resolve(self):
        documents = [ROOT / 'README.md', ROOT / 'CONTRIBUTING.md', *sorted((ROOT / 'docs').glob('*.md'))]
        checked = 0
        for document in documents:
            for target in re.findall(r'\[[^\]]+\]\(([^)]+)\)', document.read_text()):
                target = target.strip('<>').split('#', 1)[0]
                if not target or re.match(r'^[a-z]+:', target) or target.startswith('/'):
                    continue
                with self.subTest(document=str(document.relative_to(ROOT)), target=target):
                    self.assertTrue((document.parent / target).exists(), 'documentation target missing')
                checked += 1
        self.assertGreater(checked, 40)

    def test_documented_build_inputs_and_language_entrypoint_exist(self):
        for relative in ['scripts/package-release.sh', 'scripts/start-native.sh',
                         'scripts/start-docker.sh', 'runtime/sandbox/Dockerfile',
                         'runtime/sandbox/requirements.txt', 'web-console/src/i18n.ts',
                         'web-console/src/i18n-catalog.ts', 'scripts/tests/browser_i18n.mjs']:
            with self.subTest(path=relative):
                self.assertTrue((ROOT / relative).is_file())


if __name__ == '__main__':
    unittest.main()
