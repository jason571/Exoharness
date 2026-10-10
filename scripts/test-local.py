#!/usr/bin/env python3
"""Run the complete gate against the isolated local deployment database."""
import importlib.util
import os
from pathlib import Path
import subprocess
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("local_service", ROOT / "scripts" / "local-service.py")
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)
config = local.configuration()
password = (local.DATA / "postgres-password").read_text().strip()
environment = dict(os.environ)
environment["PYTHONDONTWRITEBYTECODE"] = "1"
environment["EXOHARNESS_PRO_TEST_SANDBOX_IMAGE"] = os.environ.get("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE", "skill-workspace:typescript-20260913")
environment["EXOHARNESS_PRO_TEST_DATABASE_URL"] = f"postgresql://exoharness_pro:{quote(password, safe='')}@127.0.0.1:{config['database_port']}/exoharness_pro"
raise SystemExit(subprocess.run(["bash", "scripts/test.sh"], cwd=ROOT, env=environment).returncode)
