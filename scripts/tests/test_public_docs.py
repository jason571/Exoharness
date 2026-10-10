"""Verify repository documentation links and documented local build artifacts."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class PublicDocumentationTests(unittest.TestCase):
    def test_relative_markdown_links_resolve(self):
        documents = [*sorted(ROOT.glob('README*.md')), ROOT / 'CONTRIBUTING.md', *sorted((ROOT / 'docs').glob('*.md'))]
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
                         'runtime/sandbox/requirements.txt', 'runtime/server/Dockerfile',
                         'runtime/server/Dockerfile.dockerignore', 'web-console/src/i18n.ts',
                         'web-console/src/i18n-catalog.ts', 'scripts/tests/browser_i18n.mjs']:
            with self.subTest(path=relative):
                self.assertTrue((ROOT / relative).is_file())

    def test_server_image_has_minimal_public_build_context(self):
        dockerfile = (ROOT / 'runtime/server/Dockerfile').read_text()
        self.assertIn('USER 65532:65532', dockerfile)
        copies = [line for line in dockerfile.splitlines() if line.startswith('COPY ')]
        self.assertEqual(len(copies), 1)
        self.assertIn('release/bin/exoharness /opt/exoharness/bin/exoharness', copies[0])
        allowlist = [line.strip() for line in (ROOT / 'runtime/server/Dockerfile.dockerignore').read_text().splitlines()
                     if line.strip() and not line.startswith('#')]
        self.assertEqual(allowlist, ['**', '!runtime/', '!runtime/server/',
                                    '!runtime/server/Dockerfile', '!release/',
                                    '!release/bin/', '!release/bin/exoharness'])

    def test_wechat_qr_is_only_in_chinese_readme(self):
        chinese = (ROOT / 'README.zh-CN.md').read_text()
        self.assertIn('src="docs/assets/wechat-group.jpg"', chinese)
        for document in ('README.md', 'README.en.md'):
            self.assertNotIn('wechat-group.jpg', (ROOT / document).read_text())


if __name__ == '__main__':
    unittest.main()
