import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("docker_service", Path(__file__).resolve().parents[1] / "docker-service.py")
service = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(service)

class DockerReleaseTests(unittest.TestCase):
    def test_foreign_container_is_never_stopped(self):
        raw = json.dumps([{"Config":{"Labels":{}},"State":{"Running":True}}])
        with patch.object(service.local, "docker", return_value=raw) as docker:
            with self.assertRaisesRegex(RuntimeError, "another deployment"):
                service.stop()
            self.assertEqual(docker.call_args_list[0].args, ("inspect", service.CONTAINER))
            self.assertEqual(docker.call_count, 1)

    def test_runtime_mounts_exact_release_read_only_without_secrets_in_env(self):
        config = {"management_port":9797,"portal_port":9798,"database_port":9799,"database_namespace":"execution"}
        argv = service.runtime_arguments(Path("/tmp/release/bin/exoharness"), config)
        self.assertNotIn("build", argv)
        self.assertNotIn("pull", argv)
        self.assertIn("--read-only", argv)
        self.assertIn("no-new-privileges", argv)
        self.assertIn("host", argv)
        self.assertNotIn("POSTGRES_PASSWORD=", " ".join(argv))
        self.assertIn("EXOHARNESS_MEDIA_BIND=127.0.0.1:9790",argv)
        self.assertIn("EXOHARNESS_MEDIA_PUBLIC_BASE_URL=http://127.0.0.1:9790",argv)
        for index, value in enumerate(argv):
            if value == "--mount":
                binding = argv[index+1]
                if "target=/run/exoharness/capabilities" in binding or "target=/run/exoharness/configuration" in binding:
                    self.assertNotIn(",readonly", binding)
                    self.assertIn(binding.split("target=", 1)[1], ("/run/exoharness/capabilities", "/run/exoharness/configuration"))
                else:
                    self.assertTrue(binding.endswith(",readonly"))

    def test_missing_image_never_auto_builds_or_pulls(self):
        with patch.object(service.local,"docker",return_value="") as docker:
            with self.assertRaisesRegex(RuntimeError,"no automatic pull/build"):
                service.preflight(Path("/tmp/binary"))
            self.assertEqual(docker.call_count,1)

    def test_native_service_blocks_container_switch(self):
        with patch.object(service.local,"configuration",return_value={}), patch.object(service.local,"owned_process",return_value=123), patch.object(service.local,"docker") as docker:
            with self.assertRaisesRegex(RuntimeError,"native service is running"):
                service.start()
            docker.assert_not_called()

    def test_ambiguous_docker_mount_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError,"comma"):
            service.mount(Path("/tmp/comma,path"),"/target")

if __name__ == "__main__": unittest.main()
