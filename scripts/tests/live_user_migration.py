"""Explicit real PostgreSQL migration tests, confined to one disposable schema."""
import copy
import importlib.util
import json
from pathlib import Path
import uuid
from test_user_migration import sample

spec = importlib.util.spec_from_file_location("user_migration_live", Path(__file__).parents[1] / "import-server-users.py")
migration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migration)
schema = "user_import_test_" + uuid.uuid4().hex
command = ["docker", "exec", "-i", migration.local.CONTAINER, "psql", "-X", "-q", "-A", "-t",
           "-v", "ON_ERROR_STOP=1", "-U", "exoharness_pro", "-d", "exoharness_pro"]


def sql(text):
    return migration.checked(command, text.encode())


def apply(bundle):
    return json.loads(sql(migration.import_sql(bundle).replace("auth.", schema + ".")))


try:
    sql(f"CREATE SCHEMA {schema}; CREATE TABLE {schema}.auth_users (LIKE auth.auth_users INCLUDING ALL);"
        f"CREATE TABLE {schema}.auth_user_capability_policy (LIKE auth.auth_user_capability_policy INCLUDING ALL);"
        f"ALTER TABLE {schema}.auth_user_capability_policy ADD FOREIGN KEY(user_id) REFERENCES {schema}.auth_users(id);")
    bundle = sample()
    result = apply(bundle)
    assert result["imported_users"] == result["imported_policies"] == 1
    assert apply(bundle)["imported_users"] == 0
    print("PASS atomic account + original policy import, native FK, and idempotent replay")
    new = copy.deepcopy(bundle["users"][0])
    new.update(id="user_second", username="second", normalized_username="second",
               email="second@example.org", normalized_email="second@example.org")
    for field in ["display_name", "id"]:
        conflict = copy.deepcopy(bundle)
        conflict["users"].append(new)
        conflict["users"][0][field] = "conflicting"
        try:
            apply(conflict)
            raise AssertionError("conflicting account was silently overwritten")
        except RuntimeError:
            pass
        assert int(sql(f"SELECT count(*) FROM {schema}.auth_users;")) == 1
    print("PASS changed same-ID and colliding username/email reject entire batch, including new account")
    conflict = copy.deepcopy(bundle)
    conflict["users"].append(new)
    conflict["policies"][0]["install_skills"] = 1
    try:
        apply(conflict)
        raise AssertionError("local grants were silently changed")
    except RuntimeError:
        pass
    assert int(sql(f"SELECT count(*) FROM {schema}.auth_users;")) == 1
    assert int(sql(f"SELECT install_skills FROM {schema}.auth_user_capability_policy;")) == 0
    print("PASS changed grant rejects batch; unchanged local account and permissions retained")
finally:
    # Only this script's exact UUID-owned schema; never auth or another schema.
    sql(f"DROP SCHEMA IF EXISTS {schema} CASCADE;")
