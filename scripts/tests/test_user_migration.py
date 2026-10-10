import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("user_migration", Path(__file__).parents[1] / "import-server-users.py")
migration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migration)


def sample():
    user = {"id": "user_test", "username": "test", "normalized_username": "test",
            "email": "test@example.org", "normalized_email": "test@example.org", "display_name": "Test",
            "password_hash": "$argon2id$v=19$fixture", "default_agent_profile_id": "xiaolan",
            "revision": 1, "created_at": 1, "updated_at": 1}
    policy = {"user_id": "user_test", "install_skills": 0, "install_mcp": 1, "install_tools": 0,
              "use_skills": 1, "use_mcp": 1, "use_tools": 1, "updated_at": 1}
    return {"version": 1, "users": [user], "policies": [policy]}


class UserMigrationTests(unittest.TestCase):
    def test_validation_preserves_hash_identity_grants_and_sources(self):
        bundle = sample()
        original = copy.deepcopy(bundle)
        migration.validate(bundle, {"xiaolan"})
        self.assertEqual(bundle, original)

    def test_unknown_fields_sessions_plaintext_and_missing_profile_are_rejected(self):
        for change in [lambda x: x.update(sessions=[]),
                       lambda x: x["users"][0].update(password_hash="plaintext"),
                       lambda x: x["users"][0].update(default_agent_profile_id="missing"),
                       lambda x: x["users"][0].update(revision=True)]:
            bundle = sample()
            change(bundle)
            with self.assertRaises(ValueError):
                migration.validate(bundle, {"xiaolan"})

    def test_duplicates_or_orphan_permissions_cannot_be_imported(self):
        for kind in ["users", "policies"]:
            bundle = sample()
            bundle[kind].append(copy.deepcopy(bundle[kind][0]))
            with self.assertRaises(ValueError):
                migration.validate(bundle, {"xiaolan"})
        bundle = sample()
        bundle["policies"][0]["user_id"] = "foreign"
        with self.assertRaises(ValueError):
            migration.validate(bundle, {"xiaolan"})

    def test_permissions_require_native_binary_values(self):
        for value in [True, "1", 2]:
            bundle = sample()
            bundle["policies"][0]["install_mcp"] = value
            with self.assertRaises(ValueError):
                migration.validate(bundle, {"xiaolan"})

    def test_sql_is_single_transaction_no_overwrite_no_sessions_and_payload_is_data(self):
        bundle = sample()
        bundle["users"][0]["display_name"] = "'); DROP TABLE auth.auth_users; --"
        sql = migration.import_sql(bundle)
        self.assertTrue(sql.startswith("BEGIN;"))
        self.assertTrue(sql.endswith("COMMIT;"))
        self.assertIn("LOCK TABLE", sql)
        self.assertIn("account conflict", sql)
        self.assertNotIn("ON CONFLICT DO UPDATE", sql)
        self.assertNotIn("auth_user_sessions", sql)
        self.assertNotIn(bundle["users"][0]["password_hash"], sql)
        self.assertNotIn(bundle["users"][0]["display_name"], sql)
        with self.assertRaises(ValueError):
            migration.parse('{"version":1,"version":1}')


if __name__ == "__main__":
    unittest.main()
