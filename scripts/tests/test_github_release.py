import importlib.util
from pathlib import Path, PurePosixPath
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'package-github-release.py'
SPEC = importlib.util.spec_from_file_location('github_release', SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class EvaluationReleaseTests(unittest.TestCase):
    def test_execution_modules_and_runtime_skill_are_allowed(self):
        for name in ('evaluation/modules/benchmark/pyproject.toml',
                     'evaluation/modules/evolution/src/exoharness_evolution/skill.py',
                     'evaluation/scripts/evaluate-offline.py',
                     'evaluation/scripts/evolve-skills.py',
                     'evaluation/skills/workflow-evolution/SKILL.md'):
            with self.subTest(name=name):
                self.assertTrue(MODULE.evaluation_asset(PurePosixPath(name)))

    def test_configuration_traces_notes_and_unrelated_scripts_are_denied(self):
        for name in ('evaluation/auth.json', 'evaluation/modules/benchmark/auth.json',
                     'evaluation/modules/evolution/trace.json',
                     'evaluation/modules/evolution/src/credentials.json',
                     'evaluation/scripts/private.py',
                     'evaluation/skills/workflow-evolution/notes.md',
                     'evaluation/modules/unknown/src/private.py'):
            with self.subTest(name=name):
                self.assertFalse(MODULE.evaluation_asset(PurePosixPath(name)))


if __name__ == '__main__':
    unittest.main()
