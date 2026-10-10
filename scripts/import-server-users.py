#!/usr/bin/env python3
"""Read-only server export; atomic local account import. No sessions or chats."""
import argparse
import base64
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys
import time

spec = importlib.util.spec_from_file_location("local", Path(__file__).with_name("local-service.py"))
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)
USER_FIELDS = {"id", "username", "normalized_username", "email", "normalized_email", "display_name",
               "password_hash", "default_agent_profile_id", "revision", "created_at", "updated_at"}
POLICY_FIELDS = {"user_id", "install_skills", "install_mcp", "install_tools", "use_skills", "use_mcp", "use_tools", "updated_at"}
EXPORT_SQL = """BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT json_build_object('version',1,
 'users',COALESCE((SELECT json_agg(row_to_json(u) ORDER BY id) FROM auth.auth_users u),'[]'::json),
 'policies',COALESCE((SELECT json_agg(row_to_json(p) ORDER BY user_id) FROM auth.auth_user_capability_policy p),'[]'::json));
COMMIT;"""


def parse(raw):
    def pairs(values):
        result = {}
        for key, value in values:
            if key in result:
                raise ValueError("duplicate migration field")
            result[key] = value
        return result
    if len(raw) > 16 * 1024 * 1024:
        raise ValueError("migration snapshot too large")
    return json.loads(raw, object_pairs_hook=pairs)


def validate(bundle, profiles):
    if set(bundle) != {"version", "users", "policies"} or bundle["version"] != 1:
        raise ValueError("unsupported migration snapshot")
    if not all(isinstance(bundle[k], list) for k in ("users", "policies")):
        raise ValueError("migration collections must be arrays")
    seen = {key: set() for key in ("id", "normalized_username", "normalized_email")}
    for row in bundle["users"]:
        if not isinstance(row, dict) or set(row) != USER_FIELDS:
            raise ValueError("source user schema differs")
        for key in USER_FIELDS - {"revision", "created_at", "updated_at"}:
            value = row[key]
            if not isinstance(value, str) or not value.strip() or len(value) > 1024 or any(ord(c) < 32 for c in value):
                raise ValueError("invalid source account text")
        for key in ("revision", "created_at", "updated_at"):
            if type(row[key]) is not int or not 1 <= row[key] < 2**63:
                raise ValueError("invalid source account version/time")
        if not row["password_hash"].startswith("$argon2id$v=19$"):
            raise ValueError("unsupported source password hash")
        if row["default_agent_profile_id"] not in profiles:
            raise ValueError("assigned assistant must be imported first")
        for key in seen:
            if row[key] in seen[key]:
                raise ValueError("duplicate source account identity")
            seen[key].add(row[key])
    policy_ids = set()
    for row in bundle["policies"]:
        if not isinstance(row, dict) or set(row) != POLICY_FIELDS or row["user_id"] not in seen["id"]:
            raise ValueError("invalid source permission owner/schema")
        if row["user_id"] in policy_ids:
            raise ValueError("duplicate source account permission")
        policy_ids.add(row["user_id"])
        for key in POLICY_FIELDS - {"user_id", "updated_at"}:
            if type(row[key]) is not int or row[key] not in (0, 1):
                raise ValueError("source permissions must be native 0/1")
        if type(row["updated_at"]) is not int or not 1 <= row["updated_at"] < 2**63:
            raise ValueError("invalid permission timestamp")


def checked(argv, content=None, limit=16 * 1024 * 1024):
    result = subprocess.run(argv, input=content, capture_output=True, timeout=60)
    if result.returncode or len(result.stdout) > limit:
        # Database stderr can contain account rows or hashes; never print it.
        raise RuntimeError("migration failed; private snapshot/backup retained")
    return result.stdout


def fetch(args):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", args.user) or not re.fullmatch(r"[A-Za-z0-9_.:-]+", args.host):
        raise ValueError("invalid SSH destination")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", args.remote_container) or not 1 <= args.port <= 65535:
        raise ValueError("invalid source container/port")
    reader = "import subprocess,sys\n"
    reader += "r=subprocess.run(['docker','exec','-i'," + repr(args.remote_container) + ", 'sh','-c','exec psql -X -q -A -t -v ON_ERROR_STOP=1 -U \"$POSTGRES_USER\" -d \"$POSTGRES_DB\"'],input=" + repr(EXPORT_SQL) + ",text=True,capture_output=True,timeout=30)\n"
    reader += "if r.returncode or len(r.stdout)>16777216:raise SystemExit('Read-only user export failed')\nsys.stdout.write(r.stdout)\n"
    return parse(checked(["ssh", "-i", str(args.identity), "-p", str(args.port), "-o", "BatchMode=yes",
                          "-o", "ConnectTimeout=15", f"{args.user}@{args.host}", "python3 -"], reader.encode()))


def local_sql(sql):
    return parse(checked(["docker", "exec", "-i", local.CONTAINER, "psql", "-X", "-q", "-A", "-t",
                          "-v", "ON_ERROR_STOP=1", "-U", "exoharness_pro", "-d", "exoharness_pro"], sql.encode()))


def import_sql(bundle):
    # Closed alphabet without quotes: the decoded JSON is always data.
    encoded = base64.b64encode(json.dumps(bundle, ensure_ascii=False).encode()).decode()
    return """BEGIN;
LOCK TABLE auth.auth_users,auth.auth_user_capability_policy IN SHARE ROW EXCLUSIVE MODE;
CREATE TEMP TABLE _users_input (LIKE auth.auth_users) ON COMMIT DROP;
CREATE TEMP TABLE _policy_input (LIKE auth.auth_user_capability_policy) ON COMMIT DROP;
INSERT INTO _users_input SELECT * FROM jsonb_populate_recordset(NULL::auth.auth_users,
 convert_from(decode('PAYLOAD','base64'),'UTF8')::jsonb->'users');
INSERT INTO _policy_input SELECT * FROM jsonb_populate_recordset(NULL::auth.auth_user_capability_policy,
 convert_from(decode('PAYLOAD','base64'),'UTF8')::jsonb->'policies');
DO $guard$ BEGIN
 IF EXISTS (SELECT 1 FROM _users_input i JOIN auth.auth_users u
   ON u.id=i.id OR u.normalized_username=i.normalized_username OR u.normalized_email=i.normalized_email
   WHERE to_jsonb(u)<>to_jsonb(i)) THEN RAISE EXCEPTION 'account conflict; no account overwritten'; END IF;
 IF EXISTS (SELECT 1 FROM _policy_input i JOIN auth.auth_user_capability_policy p USING(user_id)
   WHERE to_jsonb(p)<>to_jsonb(i)) THEN RAISE EXCEPTION 'permission conflict; no grant overwritten'; END IF;
END $guard$;
WITH inserted AS (INSERT INTO auth.auth_users SELECT i.* FROM _users_input i
 WHERE NOT EXISTS(SELECT 1 FROM auth.auth_users u WHERE u.id=i.id) RETURNING id),
permissions AS (INSERT INTO auth.auth_user_capability_policy SELECT i.* FROM _policy_input i
 WHERE NOT EXISTS(SELECT 1 FROM auth.auth_user_capability_policy p WHERE p.user_id=i.user_id) RETURNING user_id)
SELECT json_build_object('imported_users',(SELECT count(*) FROM inserted),
 'imported_policies',(SELECT count(*) FROM permissions),'source_users',(SELECT count(*) FROM _users_input));
COMMIT;""".replace("PAYLOAD", encoded)


def private_write(path, data):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as file:
        if os.fstat(file.fileno()).st_mode & 0o077:
            raise ValueError("migration files require 0600")
        file.write(data)
        file.flush()
        os.fsync(file.fileno())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", required=True)
    parser.add_argument("--user", required=True)
    parser.add_argument("--port", type=int, default=22)
    parser.add_argument("--identity", type=Path, required=True)
    parser.add_argument("--remote-container", default="exoharness-postgres-1")
    parser.add_argument("--apply", action="store_true", help="backup then import; otherwise inventory only")
    args = parser.parse_args()
    info = json.loads(local.docker("inspect", local.CONTAINER))[0]
    if info["Config"].get("Labels", {}).get(local.LABEL) != "1":
        raise ValueError("local database ownership mismatch")
    pointer = parse((local.DATA / "configuration/current.json").read_bytes())
    profiles = parse((local.DATA / "configuration" / pointer["generation"] / "agent-profiles.json").read_bytes())
    bundle = fetch(args)
    validate(bundle, {profile["id"] for profile in profiles["profiles"]})
    if not args.apply:
        print(json.dumps({"source_users": len(bundle["users"]), "source_policies": len(bundle["policies"]), "applied": False}))
        return
    backup = local.DATA / ("user-import-" + time.strftime("%Y%m%dT%H%M%S") + "-" + secrets.token_hex(4))
    backup.mkdir(mode=0o700)
    private_write(backup / "source-users.json", json.dumps(bundle, ensure_ascii=False).encode())
    dump = checked(["docker", "exec", local.CONTAINER, "pg_dump", "-U", "exoharness_pro", "-d", "exoharness_pro",
                    "-Fc", "--schema=auth"], limit=64 * 1024 * 1024)
    private_write(backup / "before-auth.dump", dump)
    result = local_sql(import_sql(bundle))
    current = local_sql(EXPORT_SQL)
    for group, key in (("users", "id"), ("policies", "user_id")):
        rows = {row[key]: row for row in current[group]}
        if any(rows.get(row[key]) != row for row in bundle[group]):
            raise RuntimeError("post-import verification failed; backup retained")
    result.update({"verified": True, "source_modified": False, "sessions_imported": False, "backup": str(backup)})
    private_write(backup / "report.json", json.dumps(result, ensure_ascii=False).encode())
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError):
        print("User migration failed; source untouched. No conflicting account overwritten; inspect private backup.", file=sys.stderr)
        raise SystemExit(1)
