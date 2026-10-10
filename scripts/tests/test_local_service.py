import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1] / "local-service.py"
spec = importlib.util.spec_from_file_location("local_service", SOURCE)
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)

class LocalServiceTests(unittest.TestCase):
    def test_health_requires_the_expected_service_protocol(self):
        api={"service":"exoharness","status":"ok","mode":"infrastructure_validation"}
        media={"service":"exoharness-media","status":"ok","delivery_only":True}
        for payload,expected,valid in [(api,"exoharness",True),(media,"exoharness-media",True),
            (media,"exoharness",False),(api,"exoharness-media",False),
            ({**media,"delivery_only":False},"exoharness-media",False),
            ({**media,"status":"failed"},"exoharness-media",False),([],"exoharness-media",False)]:
            with self.subTest(payload=payload,expected=expected),patch.object(local.urllib.request,"urlopen"),patch.object(local.json,"load",return_value=payload):
                self.assertEqual(local.health(9790,expected) is not None,valid)
    def test_existing_configuration_gets_media_defaults_without_changing_database_or_file(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(local,"DATA",Path(directory)):
            original={"management_port":9797,"portal_port":9798,"database_port":9799}
            local.atomic_json(Path(directory)/"config.json",original)
            config=local.configuration()
            self.assertEqual(config["media_port"],9790)
            self.assertEqual(config["media_public_base_url"],"http://127.0.0.1:9790")
            self.assertEqual(json.loads((Path(directory)/"config.json").read_text()),original)
    def test_runtime_configuration_pins_cached_image_without_build_or_pull(self):
        image_id="sha256:"+"a"*64
        with tempfile.TemporaryDirectory() as directory,patch.object(local,"DATA",Path(directory)),patch.object(local,"configuration",return_value={"management_port":9797}),patch.object(local,"docker",return_value=json.dumps([{"Id":image_id}])) as docker:
            local.set_runtime_image("existing:test")
            docker.assert_called_once_with("image","inspect","existing:test")
            self.assertEqual(json.loads((Path(directory)/"config.json").read_text()),{"management_port":9797,"sandbox_image":image_id})

    def test_bad_or_missing_runtime_never_changes_configuration(self):
        with tempfile.TemporaryDirectory() as directory,patch.object(local,"DATA",Path(directory)),patch.object(local,"atomic_json") as save:
            for image in ["", "--bad", "unsafe image"]:
                with self.assertRaises(RuntimeError):local.set_runtime_image(image)
            with patch.object(local,"docker",side_effect=RuntimeError("missing image")):
                with self.assertRaises(RuntimeError):local.set_runtime_image("missing:test")
            save.assert_not_called()

    def test_mount_namespace_alias_requires_exact_argv_start_time_and_verified_binary(self):
        import hashlib
        with tempfile.TemporaryDirectory() as directory, patch.object(local,"DATA",Path(directory)):
            binary=Path(directory)/"bin/exoharness"
            digest=hashlib.sha256(b"verified binary").hexdigest()
            local.atomic_json(Path(directory)/"service.json",{"pid":100,"executable":str(binary),"start_ticks":"10","sha256":"previous build"})
            def content(path):
                return (str(binary)+"\0").encode() if path.name=="cmdline" else b"verified binary"
            with patch.object(local,"process_identity",return_value=("/workspace/bin/exoharness","10")), patch.object(local,"verified_binary",return_value=(binary,digest)), patch.object(Path,"read_bytes",content):
                self.assertEqual(local.owned_process(),100)
                with patch.object(local,"verified_binary",return_value=(binary,"different binary")):
                    with self.assertRaisesRegex(RuntimeError,"PID identity changed"):local.owned_process()
                with patch.object(local,"process_identity",return_value=("/workspace/bin/exoharness","11")):
                    with self.assertRaisesRegex(RuntimeError,"PID identity changed"):local.owned_process()
                with patch.object(Path,"read_bytes",return_value=b"/other/process\0"):
                    with self.assertRaisesRegex(RuntimeError,"PID identity changed"):local.owned_process()

    def test_port_probe_matches_server_reuse_policy(self):
        with patch.object(local.socket,"socket") as factory:
            probe=factory.return_value.__enter__.return_value
            local.free_port(9797)
            probe.setsockopt.assert_called_once_with(local.socket.SOL_SOCKET,local.socket.SO_REUSEADDR,1)
            probe.bind.assert_called_once_with(("127.0.0.1",9797))

    def test_wsl_release_defaults_to_private_linux_state(self):
        with patch.dict(local.os.environ,{},clear=True), patch.object(local.sys,"platform","linux"), patch.object(local.Path,"home",return_value=Path("/home/tester")):
            result=local.data_directory(Path("/mnt/e/workspace/new-project"))
            self.assertTrue(str(result).startswith("/home/tester/.local/state/exoharness/"))

    def test_source_release_shares_data_but_standalone_package_does_not(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(local.os.environ,{},clear=True):
            root=Path(directory);(root/"Cargo.toml").write_text("[workspace]")
            self.assertEqual(local.data_directory(root/"release"),root/"data"/"local")
            self.assertEqual(local.data_directory(root/"standalone"),root/"standalone"/"data"/"local")

    def test_private_atomic_config_is_not_world_readable(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/"config.json"
            local.atomic_json(path,{"test":True})
            self.assertEqual(json.loads(path.read_text()),{"test":True})
            self.assertEqual(path.stat().st_mode & 0o777,0o600)
            self.assertEqual(list(path.parent.glob("*.tmp-*")),[])

    def test_pid_reuse_is_rejected_before_any_signal(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(local,"DATA",Path(directory)):
            local.atomic_json(Path(directory)/"service.json",{"pid":100,"executable":"/expected","start_ticks":"10"})
            with patch.object(local,"process_identity",return_value=("/other","11")):
                with self.assertRaisesRegex(RuntimeError,"PID identity changed"):
                    local.owned_process()

    def test_missing_pid_does_not_stop_other_services(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(local,"DATA",Path(directory)):
            with patch.object(local.os,"kill") as kill:
                local.stop_service()
                kill.assert_not_called()

    def test_binary_manifest_mismatch_is_rejected_before_spawn(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/"bin").mkdir();(root/"bin"/"exoharness").write_bytes(b"not a service")
            (root/"build-manifest.json").write_text(json.dumps({"local_test_service_ready":True,"binaries":[{"file":"bin/exoharness","sha256":"wrong"}]}))
            with patch.object(local,"RELEASE",root), patch.object(local,"configuration",return_value={}), patch.object(local,"owned_process",return_value=None),patch.object(local.subprocess,"Popen") as spawn:
                with self.assertRaisesRegex(RuntimeError,"does not match"):
                    local.start_service()
                spawn.assert_not_called()

if __name__=="__main__":unittest.main()
