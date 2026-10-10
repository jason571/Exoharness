"""Owned ephemeral account -> original portal login -> identity and ASR browser.
No real account password reset and no remote operation. Removes only its user.
"""
import base64
import ctypes
import ctypes.util
import importlib.util
import json
import os
from pathlib import Path
import secrets
import re
import subprocess
import time
import uuid
import sys

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("portal_local", ROOT / "scripts/local-service.py")
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)
uid = "portal_browser_" + uuid.uuid4().hex
namespace = local.configuration()["database_namespace"]
if not re.fullmatch(r"[a-zA-Z_][a-zA-Z0-9_]{0,62}", namespace):
    raise RuntimeError("invalid test database namespace")
password = secrets.token_urlsafe(24)
salt = os.urandom(16)
library = ctypes.util.find_library("argon2")
if not library:
    raise RuntimeError("Test fixture requires local libargon2; no real user password is modified")
hash_raw = ctypes.CDLL(library).argon2id_hash_raw
hash_raw.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32,
                    ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t,
                    ctypes.c_void_p, ctypes.c_size_t]
hash_raw.restype = ctypes.c_int
digest_buffer = ctypes.create_string_buffer(32)
if hash_raw(2, 19456, 1, password.encode(), len(password.encode()), salt, len(salt), digest_buffer, 32):
    raise RuntimeError("Test fixture Argon2id failed")
digest = digest_buffer.raw
b64 = lambda value: base64.b64encode(value).decode().rstrip("=")
phc = f"$argon2id$v=19$m=19456,t=2,p=1${b64(salt)}${b64(digest)}"


def sql(text):
    result = subprocess.run(["docker", "exec", "-i", local.CONTAINER, "psql", "-X", "-q", "-A", "-t",
                             "-v", "ON_ERROR_STOP=1", "-U", "exoharness_pro", "-d", "exoharness_pro"],
                            input=text, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError("fixture database operation failed")
    return result.stdout.strip()


try:
    bundle = json.dumps({"id": uid, "hash": phc, "now": int(time.time())})
    encoded = base64.b64encode(bundle.encode()).decode()
    sql(f"""INSERT INTO auth.auth_users(id,username,normalized_username,email,normalized_email,display_name,password_hash,default_agent_profile_id,revision,created_at,updated_at)
        SELECT j->>'id', j->>'id', j->>'id',(j->>'id')||'@example.invalid',(j->>'id')||'@example.invalid','Browser test',j->>'hash','xiaolan',1,(j->>'now')::bigint,(j->>'now')::bigint
        FROM (SELECT convert_from(decode('{encoded}','base64'),'UTF8')::jsonb j) value;""")
    env = dict(os.environ, EXOHARNESS_BROWSER_USER=uid, EXOHARNESS_BROWSER_PASSWORD=password)
    arguments = ["--settings-only"] if "--settings-only" in sys.argv else []
    subprocess.run(["node", "scripts/tests/browser_portal_assistant.mjs", *arguments], cwd=ROOT, env=env, check=True)
finally:
    # Only this test's owned records; no existing user's data is touched.
    sql(f"DELETE FROM {namespace}.knowledge_documents WHERE owner_user_id='{uid}'; DELETE FROM {namespace}.knowledge_categories WHERE scope_owner='{uid}'; DELETE FROM {namespace}.personal_memories WHERE owner_user_id='{uid}'; DELETE FROM {namespace}.execution_sessions WHERE owner_user_id='{uid}';")
    sql(f"DELETE FROM auth.auth_users WHERE id='{uid}'; DELETE FROM auth.auth_login_attempts WHERE bucket='portal-login:{uid}';")
