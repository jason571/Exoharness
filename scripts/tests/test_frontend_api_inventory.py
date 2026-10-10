import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("frontend_inventory", ROOT / "scripts/frontend-api-inventory.py")
inventory = importlib.util.module_from_spec(spec)
spec.loader.exec_module(inventory)


class FrontendInventoryTests(unittest.TestCase):
    def test_assigned_assistant_and_speech_paths_have_real_routes(self):
        entries = {item["path"]: item for item in inventory.inventory()["entries"]}
        for path in ["/v1/agent-profile", "/v1/speech-input", "/v1/speech-input/configuration", "/v1/application-info"]:
            self.assertEqual(entries[path]["status"], "declared")

    def test_template_and_missing_routes_are_not_silently_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "modules/api/src").mkdir(parents=True)
            (root / "web-console/src").mkdir(parents=True)
            (root / "web").mkdir()
            (root / "modules/api/src/routes.rs").write_text('.route("/v1/items/{id}", get(handler))')
            (root / "web-console/src/test.ts").write_text('api(`/v1/items/${id}?limit=5`); api("/v1/missing");')
            report = inventory.inventory(root)
            self.assertEqual(report["declared"], 1)
            self.assertEqual(report["missing"], 1)
            self.assertEqual(report["scope"], "static_path_coverage_only")


if __name__ == "__main__":
    unittest.main()
