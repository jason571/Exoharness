"""Architecture guardrails, not functional feature acceptance.

Checks source boundaries without starting a service, database or model.
"""
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "modules"


class ModuleStructureTests(unittest.TestCase):
    def test_required_components_have_separate_directories(self):
        for name in ("contracts", "core", "agent", "api", "app", "configuration",
                     "identity", "registry", "skills", "mcp", "coding", "sandbox", "model-runtime", "skill-runtime", "artifacts"):
            with self.subTest(module=name):
                self.assertTrue((MODULES / name).is_dir())
        # A directory is a structure check only, never evidence of readiness.

    def test_local_crate_graph_is_acyclic_and_foundations_do_not_depend_on_adapters(self):
        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
        manifests = {}
        for member in workspace["workspace"]["members"]:
            path = ROOT / member / "Cargo.toml"
            value = tomllib.loads(path.read_text())
            manifests[value["package"]["name"]] = value
        names = set(manifests)
        edges = {
            name: names.intersection(value.get("dependencies", {}))
            for name, value in manifests.items()
        }
        self.assertEqual(edges["exoharness-contracts"], set())
        self.assertLessEqual(edges["exoharness-core"], {"exoharness-contracts"})
        self.assertNotIn("exoharness-app", set().union(*edges.values()))

        visited, active = set(), set()

        def visit(name):
            self.assertNotIn(name, active, f"cyclic module dependency at {name}")
            if name in visited:
                return
            active.add(name)
            for dependency in edges[name]:
                visit(dependency)
            active.remove(name)
            visited.add(name)

        for name in edges:
            visit(name)

    def test_rust_test_implementations_are_outside_production_source(self):
        for path in MODULES.rglob("*.rs"):
            if "src" not in path.relative_to(MODULES).parts:
                continue
            source = path.read_text()
            with self.subTest(file=str(path.relative_to(ROOT))):
                self.assertIsNone(re.search(r"#\[(?:tokio::)?test(?:\([^]]*\))?\]", source))
                self.assertIsNone(re.search(r"\bmod\s+tests\s*\{", source))
                for attributes in re.findall(r'((?:#\[[^\]]+\]\s*)+)mod\s+\w+\s*;', source):
                    match = re.search(r'#\[path\s*=\s*"([^"]+)"\]', attributes)
                    if match is None:
                        continue
                    target = match[1]
                    destination = (path.parent / target).resolve()
                    test_only = bool(re.search(r'#\[cfg\(test\)\]', attributes))
                    self.assertEqual("tests" in destination.relative_to(MODULES).parts, test_only,
                                     f"module/test boundary mismatch: {target}")
                    self.assertTrue(destination.is_file(), f"missing module file: {target}")

    def test_path_module_attributes_distinguish_test_and_production(self):
        source = '#[path = "project.rs"]\nmod project;\n#[cfg(test)]\n#[path = "../tests/check.rs"]\nmod tests;'
        groups = re.findall(r'((?:#\[[^\]]+\]\s*)+)mod\s+\w+\s*;', source)
        self.assertEqual(len(groups), 2)
        self.assertNotRegex(groups[0], r'#\[cfg\(test\)\]')
        self.assertRegex(groups[1], r'#\[cfg\(test\)\]')

    def test_production_modules_use_explicit_local_imports(self):
        for path in MODULES.rglob("*.rs"):
            if "src" not in path.relative_to(MODULES).parts:
                continue
            source = path.read_text()
            with self.subTest(file=str(path.relative_to(ROOT))):
                self.assertIsNone(re.search(r"\buse\s+(?:super|crate)::\*\s*;", source))
                self.assertNotIn("validation::*", source)

    def test_configuration_entrypoints_only_compose_responsibility_modules(self):
        required = {
            "configuration/src/models/mod.rs":
                {"catalog", "credentials", "defaults", "kinds", "parameters", "providers", "speech", "persistence", "mutations", "selection", "runtime"},
            "configuration/src/profiles/mod.rs":
                {"access", "avatars", "projection", "validation", "persistence", "mutations", "assignment"},
            "api/src/configuration/mod.rs": {"avatars", "models", "profiles", "speech", "model_requests", "model_test", "model_writes", "assistant_requests", "assistant_writes", "assistant_authorization", "scheduler"},
        }
        for relative, children in required.items():
            path = MODULES / relative
            with self.subTest(module=relative):
                source = path.read_text()
                declared = set(re.findall(r"^mod\s+(\w+)\s*;", source, re.M))
                self.assertEqual(declared, children)
                self.assertNotIn("async fn", source)
                self.assertNotIn("SELECT ", source)
                for child in children:
                    self.assertTrue((path.parent / f"{child}.rs").is_file())
        for legacy in ("configuration/src/models.rs", "configuration/src/profiles.rs",
                       "api/src/configuration.rs"):
            self.assertFalse((MODULES / legacy).exists(), f"duplicate module entry: {legacy}")


if __name__ == "__main__":
    unittest.main()
