use exoharness_identity::{
    AuthStore, Database,
    external::{
        ApplicationInput, AuthorizationRequest, IssuedApplication, IssuedTokens, pkce_challenge,
    },
};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicU64, Ordering},
};
const VERIFIER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ1234567890";
#[test]
fn consent_transaction_rechecks_cookie_ownership_logout_and_password_revision() {
    let f = fixture();
    f.store.create_external_application(input("a")).unwrap();
    let session = f
        .store
        .authenticate_user("external_user", "account-password", 3600)
        .unwrap()
        .unwrap();
    assert!(
        f.store
            .authorize_external_application_from_session(
                "another-user",
                &session.token,
                &request("a")
            )
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .authorize_external_application_from_session(&f.user, &session.token, &request("a"))
            .unwrap()
            .is_some()
    );
    let digest = f
        .store
        .validate_user_session(&session.token)
        .unwrap()
        .unwrap()
        .token_digest;
    f.store.delete_user_session_digest(&digest).unwrap();
    assert!(
        f.store
            .authorize_external_application_from_session(&f.user, &session.token, &request("a"))
            .unwrap()
            .is_none()
    );
    let session = f
        .store
        .authenticate_user("external_user", "account-password", 3600)
        .unwrap()
        .unwrap();
    f.store
        .update_user_password(&f.user, "account-password", "replacement-password")
        .unwrap();
    assert!(
        f.store
            .authorize_external_application_from_session(&f.user, &session.token, &request("a"))
            .unwrap()
            .is_none()
    );
}
struct Fixture {
    store: AuthStore,
    db: Database,
    user: String,
}
fn fixture() -> Fixture {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let namespace = format!(
        "pro_ext_identity_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let db = Database::connect(
        &std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").unwrap(),
        None,
        &namespace,
    )
    .unwrap();
    let store = AuthStore::open(db.clone()).unwrap();
    let code = store
        .issue_registration_code("external@example.com")
        .unwrap()
        .unwrap();
    store.activate_registration_code(&code.id).unwrap();
    let user = store
        .register_verified_user(
            &code.email,
            &code.code,
            "external_user",
            "External user",
            "account-password",
            3600,
        )
        .unwrap()
        .account
        .id;
    Fixture { store, db, user }
}
fn input(id: &str) -> ApplicationInput {
    ApplicationInput {
        client_id: id.into(),
        name: id.into(),
        assistant_id: format!("assistant_{id}"),
        redirect_uris: vec!["https://app.example/callback".into()],
        scopes: vec![
            "identity:read".into(),
            "sessions:read".into(),
            "sessions:write".into(),
            "runs:read".into(),
        ],
        enabled: true,
        access_ttl_seconds: 300,
        grant_ttl_seconds: 3600,
        require_device_id: false,
        allow_backend_identity: true,
    }
}
fn request(id: &str) -> AuthorizationRequest {
    AuthorizationRequest {
        client_id: id.into(),
        redirect_uri: "https://app.example/callback".into(),
        scope: "identity:read sessions:read sessions:write runs:read".into(),
        state: "random-state-1234567890".into(),
        code_challenge: pkce_challenge(VERIFIER).unwrap(),
        code_challenge_method: "S256".into(),
        response_type: "code".into(),
    }
}
fn code(f: &Fixture, id: &str) -> String {
    let redirect = f
        .store
        .authorize_external_application(&f.user, &request(id))
        .unwrap();
    url::Url::parse(&redirect)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned()
}
fn tokens(f: &Fixture, app: &IssuedApplication) -> IssuedTokens {
    f.store
        .exchange_external_code(
            &app.application,
            &code(f, &app.application.input.client_id),
            "https://app.example/callback",
            VERIFIER,
        )
        .unwrap()
        .unwrap()
}

#[test]
fn legacy_application_keys_survive_reopen_and_rotate_only_when_requested() {
    let f = fixture();
    let legacy = format!("exs_{}", "ab".repeat(32));
    let app = f
        .store
        .create_external_application_with_key(input("legacy"), legacy.clone(), String::new())
        .unwrap();
    let grant = tokens(&f, &app);
    let reopened = AuthStore::open(f.db.clone()).unwrap();
    assert!(
        reopened
            .authenticate_external_application("legacy", &legacy)
            .unwrap()
            .is_some()
    );
    assert!(
        reopened
            .validate_external_access(&grant.access_token)
            .unwrap()
            .is_some()
    );
    let rotated = reopened
        .update_external_application(input("legacy"), 1, true)
        .unwrap();
    assert!(rotated.client_secret.starts_with("exo_"));
    assert!(
        reopened
            .authenticate_external_application("legacy", &legacy)
            .unwrap()
            .is_none()
    );
    assert!(
        reopened
            .authenticate_external_application("legacy", &rotated.client_secret)
            .unwrap()
            .is_some()
    );
}

#[test]
fn multiple_applications_share_user_identity_and_keep_independent_roles_and_grants() {
    let f = fixture();
    let a = f.store.create_external_application(input("a")).unwrap();
    let b = f.store.create_external_application(input("b")).unwrap();
    assert!(a.client_secret.starts_with("exo_"));
    assert!(b.client_secret.starts_with("exo_"));
    let ta = tokens(&f, &a);
    let tb = tokens(&f, &b);
    let ga = f
        .store
        .validate_external_access(&ta.access_token)
        .unwrap()
        .unwrap();
    let gb = f
        .store
        .validate_external_access(&tb.access_token)
        .unwrap()
        .unwrap();
    assert_eq!(ga.user_id, gb.user_id);
    assert_ne!(ga.id, gb.id);
    assert_eq!(ga.assistant_id, "assistant_a");
    assert_eq!(gb.assistant_id, "assistant_b");
    assert!(
        !serde_json::to_string(&f.store.external_applications().unwrap())
            .unwrap()
            .contains(&a.client_secret)
    );
    assert!(
        f.store
            .authenticate_external_application("a", &b.client_secret)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .validate_user_session(&ta.access_token)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .validate_external_access(&ta.refresh_token)
            .unwrap()
            .is_none()
    );
    assert!(
        !f.store
            .revoke_external_application_grant("b", &ga.id)
            .unwrap()
    );
    assert!(
        !f.store
            .revoke_external_grant("another-user", &ga.id)
            .unwrap()
    );
    assert!(f.store.revoke_external_grant(&f.user, &ga.id).unwrap());
    assert!(
        f.store
            .validate_external_access(&ta.access_token)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .validate_external_access(&tb.access_token)
            .unwrap()
            .is_some()
    );
    assert!(
        AuthStore::open(f.db.clone())
            .unwrap()
            .validate_external_access(&tb.access_token)
            .unwrap()
            .is_some()
    );
}
#[test]
fn authorization_requires_exact_redirect_scope_and_pkce_and_consumes_code_once() {
    let f = fixture();
    let a = f.store.create_external_application(input("a")).unwrap();
    let b = f.store.create_external_application(input("b")).unwrap();
    for mutation in 0..4 {
        let mut r = request("a");
        match mutation {
            0 => r.redirect_uri = "https://attacker.example/callback".into(),
            1 => r.scope = "admin:all".into(),
            2 => r.code_challenge_method = "plain".into(),
            _ => r.state = "short".into(),
        };
        assert!(f.store.validate_external_authorization(&r).is_err());
    }
    let code = code(&f, "a");
    assert!(
        f.store
            .exchange_external_code(
                &b.application,
                &code,
                "https://app.example/callback",
                VERIFIER
            )
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .exchange_external_code(&a.application, &code, "https://app.example/other", VERIFIER)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .exchange_external_code(
                &a.application,
                &code,
                "https://app.example/callback",
                &"x".repeat(43)
            )
            .unwrap()
            .is_none()
    );
    let issued = f
        .store
        .exchange_external_code(
            &a.application,
            &code,
            "https://app.example/callback",
            VERIFIER,
        )
        .unwrap()
        .unwrap();
    assert!(
        f.store
            .exchange_external_code(
                &a.application,
                &code,
                "https://app.example/callback",
                VERIFIER
            )
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .validate_external_access(&issued.access_token)
            .unwrap()
            .is_none()
    );
}
#[test]
fn refresh_replay_revokes_its_family_and_preserves_other_application() {
    let f = fixture();
    let a = f.store.create_external_application(input("a")).unwrap();
    let b = f.store.create_external_application(input("b")).unwrap();
    let ta = tokens(&f, &a);
    let tb = tokens(&f, &b);
    assert!(
        f.store
            .refresh_external_token(&b.application, &ta.refresh_token)
            .unwrap()
            .is_none()
    );
    let next = f
        .store
        .refresh_external_token(&a.application, &ta.refresh_token)
        .unwrap()
        .unwrap();
    assert_ne!(next.access_token, ta.access_token);
    assert!(
        f.store
            .validate_external_access(&next.access_token)
            .unwrap()
            .is_some()
    );
    assert!(
        f.store
            .refresh_external_token(&a.application, &ta.refresh_token)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .validate_external_access(&next.access_token)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .validate_external_access(&tb.access_token)
            .unwrap()
            .is_some()
    );
}
#[test]
fn key_rotation_disable_and_password_change_revoke_only_the_expected_authorizations() {
    let f = fixture();
    let a = f.store.create_external_application(input("a")).unwrap();
    let b = f.store.create_external_application(input("b")).unwrap();
    let ta = tokens(&f, &a);
    let tb = tokens(&f, &b);
    assert!(
        f.store
            .update_external_application(input("a"), 99, true)
            .is_err()
    );
    assert!(
        f.store
            .validate_external_access(&ta.access_token)
            .unwrap()
            .is_some()
    );
    let next = f
        .store
        .update_external_application(input("a"), 1, true)
        .unwrap();
    assert!(next.client_secret.starts_with("exo_"));
    assert_ne!(a.client_secret, next.client_secret);
    assert!(
        f.store
            .authenticate_external_application("a", &a.client_secret)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .validate_external_access(&ta.access_token)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .validate_external_access(&tb.access_token)
            .unwrap()
            .is_some()
    );
    let mut disabled = input("a");
    disabled.enabled = false;
    f.store
        .update_external_application(disabled, 2, false)
        .unwrap();
    assert!(
        f.store
            .authenticate_external_application("a", &next.client_secret)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .update_user_password(&f.user, "account-password", "new-account-password")
            .unwrap()
    );
    assert!(
        f.store
            .validate_external_access(&tb.access_token)
            .unwrap()
            .is_none()
    );
}
#[test]
fn expired_access_code_and_grant_are_never_accepted() {
    let f = fixture();
    let a = f.store.create_external_application(input("a")).unwrap();
    let code = code(&f, "a");
    f.db.connection()
        .unwrap()
        .execute_batch("UPDATE auth_external_codes SET expires_at=0")
        .unwrap();
    assert!(
        f.store
            .exchange_external_code(
                &a.application,
                &code,
                "https://app.example/callback",
                VERIFIER
            )
            .unwrap()
            .is_none()
    );
    let t = tokens(&f, &a);
    f.db.connection()
        .unwrap()
        .execute_batch("UPDATE auth_external_tokens SET expires_at=0 WHERE kind='access'")
        .unwrap();
    assert!(
        f.store
            .validate_external_access(&t.access_token)
            .unwrap()
            .is_none()
    );
    let next = f
        .store
        .refresh_external_token(&a.application, &t.refresh_token)
        .unwrap()
        .unwrap();
    f.db.connection()
        .unwrap()
        .execute_batch("UPDATE auth_external_grants SET expires_at=0")
        .unwrap();
    assert!(
        f.store
            .validate_external_access(&next.access_token)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .refresh_external_token(&a.application, &next.refresh_token)
            .unwrap()
            .is_none()
    );
}
#[test]
fn concurrent_refresh_reuse_cannot_leave_a_second_valid_token_family() {
    let f = fixture();
    let a = f.store.create_external_application(input("a")).unwrap();
    let t = tokens(&f, &a);
    let store = Arc::new(f.store);
    let barrier = Arc::new(Barrier::new(2));
    let jobs = (0..2)
        .map(|_| {
            let store = store.clone();
            let app = a.application.clone();
            let token = t.refresh_token.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.refresh_external_token(&app, &token).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let results = jobs
        .into_iter()
        .filter_map(|j| j.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert!(
        store
            .validate_external_access(&results[0].access_token)
            .unwrap()
            .is_none()
    );
}

#[test]
fn deleting_application_revokes_every_credential_and_retains_users_and_other_applications() {
    let f = fixture();
    let a = f
        .store
        .create_external_application_with_key(
            input("a"),
            format!("exo_{}", "fixture".repeat(8)),
            "encrypted-fixture".into(),
        )
        .unwrap();
    let b = f.store.create_external_application(input("b")).unwrap();
    let ta = tokens(&f, &a);
    let tb = tokens(&f, &b);
    let pending_code = code(&f, "a");
    let backend = f
        .store
        .external_backend_tokens(
            "a",
            &a.client_secret,
            "app-user",
            Some("Alice"),
            Some("watch-a"),
        )
        .unwrap()
        .unwrap();
    let mapped_user = backend.user_id.clone();
    let references = f.store.assistant_reference_count("assistant_a").unwrap();
    assert!(
        f.store
            .delete_external_application("a", 0, "admin")
            .is_err()
    );
    assert!(
        f.store
            .delete_external_application("a", 99, "admin")
            .is_err()
    );
    assert!(
        f.store
            .validate_external_access(&ta.access_token)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        f.store.external_encrypted_key("a").unwrap().as_deref(),
        Some("encrypted-fixture")
    );
    assert!(
        f.store
            .delete_external_application("a", 1, "admin")
            .unwrap()
    );
    assert!(
        !f.store
            .delete_external_application("a", 1, "admin")
            .unwrap()
    );
    assert!(
        !f.store
            .delete_external_application("missing", 1, "admin")
            .unwrap()
    );
    assert!(f.store.external_application("a").unwrap().is_none());
    assert_eq!(f.store.external_applications().unwrap().len(), 1);
    assert!(f.store.external_encrypted_key("a").unwrap().is_none());
    assert!(
        f.store
            .authenticate_external_application("a", &a.client_secret)
            .unwrap()
            .is_none()
    );
    for token in [&ta.access_token, &backend.access_token] {
        assert!(f.store.validate_external_access(token).unwrap().is_none());
    }
    assert!(
        f.store
            .refresh_external_token(&a.application, &ta.refresh_token)
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .exchange_external_code(
                &a.application,
                &pending_code,
                "https://app.example/callback",
                VERIFIER
            )
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .external_backend_tokens("a", &a.client_secret, "app-user", None, Some("watch-a"))
            .unwrap()
            .is_none()
    );
    assert!(
        f.store
            .external_backend_grant("a", &a.client_secret, "app-user", None, Some("watch-a"))
            .unwrap()
            .is_none()
    );
    assert!(f.store.user_account(&f.user).unwrap().is_some());
    assert!(f.store.user_account(&mapped_user).unwrap().is_some());
    assert_eq!(
        f.store
            .external_user_projection("a", &mapped_user)
            .unwrap()
            .unwrap()
            .0,
        "app-user"
    );
    assert_eq!(
        f.store.assistant_reference_count("assistant_a").unwrap(),
        references - 1
    );
    assert!(
        f.store
            .authenticate_external_application("b", &b.client_secret)
            .unwrap()
            .is_some()
    );
    assert!(
        f.store
            .validate_external_access(&tb.access_token)
            .unwrap()
            .is_some()
    );
    assert!(
        f.store
            .update_external_application(input("a"), 2, false)
            .is_err()
    );
    assert!(f.store.create_external_application(input("a")).is_err());
    let connection = f.db.connection().unwrap();
    let tombstone:(i64,String,String,i64) = connection.query_row("SELECT revision,secret_digest,encrypted_key,deleted_at FROM auth_external_applications WHERE client_id='a'", (), |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(tombstone.0, 2);
    assert!(tombstone.1.is_empty() && tombstone.2.is_empty());
    assert!(tombstone.3 > 0);
    let audits: i64 = connection
        .query_row(
            "SELECT count(*) FROM auth_external_key_audit WHERE client_id='a' AND action='delete'",
            (),
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audits, 1);
    let reopened = AuthStore::open(f.db.clone()).unwrap();
    assert!(reopened.external_application("a").unwrap().is_none());
    assert!(
        reopened
            .validate_external_access(&tb.access_token)
            .unwrap()
            .is_some()
    );
}

#[test]
fn concurrent_application_edit_and_delete_cannot_commit_the_same_revision() {
    let f = fixture();
    let app = f.store.create_external_application(input("a")).unwrap();
    let store = std::sync::Arc::new(f.store);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let edit = {
        let s = store.clone();
        let b = barrier.clone();
        std::thread::spawn(move || {
            b.wait();
            s.update_external_application(input("a"), 1, false).is_ok()
        })
    };
    let delete = {
        let s = store.clone();
        let b = barrier.clone();
        std::thread::spawn(move || {
            b.wait();
            s.delete_external_application("a", 1, "admin")
                .unwrap_or(false)
        })
    };
    barrier.wait();
    let edited = edit.join().unwrap();
    let deleted = delete.join().unwrap();
    assert_ne!(edited, deleted);
    if deleted {
        assert!(store.external_application("a").unwrap().is_none());
        assert!(
            store
                .authenticate_external_application("a", &app.client_secret)
                .unwrap()
                .is_none()
        );
    } else {
        assert_eq!(
            store.external_application("a").unwrap().unwrap().revision,
            2
        );
        assert!(
            store
                .authenticate_external_application("a", &app.client_secret)
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn failed_deletion_rolls_back_key_grants_revision_and_audit() {
    let f = fixture();
    let a = f
        .store
        .create_external_application_with_key(
            input("a"),
            format!("exo_{}", "fixture".repeat(8)),
            "encrypted-fixture".into(),
        )
        .unwrap();
    let token = tokens(&f, &a);
    let connection = f.db.connection().unwrap();
    connection.execute_batch("CREATE FUNCTION reject_delete_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='delete' THEN RAISE EXCEPTION 'fixture audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_delete_audit BEFORE INSERT ON auth_external_key_audit FOR EACH ROW EXECUTE FUNCTION reject_delete_audit();").unwrap();
    assert!(
        f.store
            .delete_external_application("a", 1, "admin")
            .is_err()
    );
    assert_eq!(
        f.store.external_application("a").unwrap().unwrap().revision,
        1
    );
    assert_eq!(
        f.store.external_encrypted_key("a").unwrap().as_deref(),
        Some("encrypted-fixture")
    );
    assert!(
        f.store
            .authenticate_external_application("a", &a.client_secret)
            .unwrap()
            .is_some()
    );
    assert!(
        f.store
            .validate_external_access(&token.access_token)
            .unwrap()
            .is_some()
    );
    connection.execute_batch("DROP TRIGGER reject_delete_audit ON auth_external_key_audit; DROP FUNCTION reject_delete_audit();").unwrap();
    assert!(
        f.store
            .delete_external_application("a", 1, "admin")
            .unwrap()
    );
}

#[test]
fn deletion_migration_preserves_existing_accounts_keys_and_grants() {
    let f = fixture();
    let a = f.store.create_external_application(input("a")).unwrap();
    let token = tokens(&f, &a);
    f.db.connection().unwrap().execute_batch("ALTER TABLE auth_external_applications DROP COLUMN deleted_at; DELETE FROM auth_external_migrations WHERE version=4;").unwrap();
    let upgraded = AuthStore::open(f.db.clone()).unwrap();
    assert!(upgraded.user_account(&f.user).unwrap().is_some());
    assert_eq!(
        upgraded
            .external_application("a")
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    assert!(
        upgraded
            .authenticate_external_application("a", &a.client_secret)
            .unwrap()
            .is_some()
    );
    assert!(
        upgraded
            .validate_external_access(&token.access_token)
            .unwrap()
            .is_some()
    );
    assert!(
        upgraded
            .delete_external_application("a", 1, "admin")
            .unwrap()
    );
    assert!(
        upgraded
            .validate_external_access(&token.access_token)
            .unwrap()
            .is_none()
    );
}
