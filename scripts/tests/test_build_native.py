import argparse
import importlib.util
import json
from pathlib import Path
import unittest
import tempfile
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "build-native.py"
SPEC = importlib.util.spec_from_file_location("build_native", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

class NativeBuildTests(unittest.TestCase):
    def test_skip_web_packages_current_prebuilt_assets_not_stale_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);output=root/'release';binary=root/'target/exoharness'
            for path,content in ((root/'web/chat.js','current asset'),(output/'web/chat.js','stale asset'),(root/'docs/RELEASE.txt','release'),(binary,'binary')):
                path.parent.mkdir(parents=True,exist_ok=True);path.write_text(content)
            for name in MODULE.PACKAGE_DOCUMENTS:
                path=root/name
                path.parent.mkdir(parents=True,exist_ok=True)
                if not path.exists():path.write_text('public package document')
            calls=[]
            def command(argv,**kwargs):
                calls.append(argv)
                if argv[0]=='cargo':return json.dumps({'reason':'compiler-artifact','target':{'kind':['bin']},'executable':str(binary),'profile':{'test':False}})
                if argv==['rustc','-vV']:return 'host: x86_64-unknown-linux-gnu\n'
                return 'fixture compiler'
            with patch.object(MODULE,'ROOT',root),patch.object(MODULE,'command',command),patch.object(MODULE.shutil,'which',return_value='/fixture/tool'),patch.object(MODULE.sys,'argv',['build-native.py','--skip-web','--output',str(output)]):
                self.assertEqual(MODULE.main(),0)
            self.assertEqual((output/'web/chat.js').read_text(),'current asset')
            manifest=json.loads((output/'build-manifest.json').read_text())
            self.assertEqual(manifest['status'], 'Native release packaged; target-host ABI and deployment verification required')
            self.assertFalse(manifest['deployment_ready'])
            asset=next(item for item in manifest['files'] if item['file']=='web/chat.js')
            self.assertEqual(asset['sha256'],MODULE.checksum(root/'web/chat.js'))
            for source,name in MODULE.PACKAGE_DOCUMENTS.items():
                self.assertEqual((output/name).read_bytes(),(root/source).read_bytes())
                self.assertIn(name,[item['file'] for item in manifest['files']])
            self.assertFalse(any(argv[0] in ('npm','docker') for argv in calls))

    def test_release_inventory_excludes_data_secrets_and_cache(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for name in ("bin/exoharness","web/index.html","scripts/start-native.sh","scripts/__pycache__/code.pyc","data/credentials.json"):
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text("test")
            files=[item["file"] for item in MODULE.release_inventory(root)]
            self.assertEqual(files,["bin/exoharness","scripts/start-native.sh","web/index.html"])

    def test_native_build_never_invokes_docker(self):
        args = argparse.Namespace(profile="release", target="x86_64-unknown-linux-musl", offline=True)
        argv = MODULE.build_options(args)
        self.assertEqual(argv[0], "cargo")
        self.assertIn("--locked", argv)
        self.assertIn("--offline", argv)
        self.assertIn("--release", argv)
        self.assertNotIn("docker", argv)

    def test_dev_build_preserves_cargo_default_target(self):
        args = argparse.Namespace(profile="dev", target=None, offline=False)
        argv = MODULE.build_options(args)
        self.assertNotIn("--release", argv)
        self.assertNotIn("--target", argv)

    def test_library_and_test_outputs_are_not_published_as_service_binaries(self):
        events = [
            {"reason":"compiler-artifact", "target":{"kind":["lib"]}, "executable":None},
            {"reason":"compiler-artifact", "target":{"kind":["bin"]}, "executable":"/tmp/test", "profile":{"test":True}},
            {"reason":"compiler-artifact", "target":{"kind":["bin"]}, "executable":"/tmp/server", "profile":{"test":False}},
            {"reason":"build-finished", "success":True},
        ]
        self.assertEqual(MODULE.binary_artifacts("\n".join(map(json.dumps,events))), [Path("/tmp/server")])

if __name__ == "__main__":
    unittest.main()
