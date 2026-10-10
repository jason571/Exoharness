use exoharness_identity::{
    AuthRole, AuthStore, DEFAULT_SESSION_TTL_SECONDS, Database, UserCapabilityPolicy,
};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicU64, Ordering},
};
fn database() -> Database {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let namespace = format!(
        "pro_identity_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let url =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated PostgreSQL required");
    Database::connect(&url, None, &namespace).unwrap()
}
#[test]
fn password_bootstrap_never_undoes_a_rotation_and_session_survives_reopen() {
    let database = database();
    let store = AuthStore::open(database.clone()).unwrap();
    store
        .bootstrap_password(AuthRole::Admin, "initial-password")
        .unwrap();
    let session = store
        .authenticate_session("initial-password", DEFAULT_SESSION_TTL_SECONDS)
        .unwrap()
        .unwrap();
    assert!(
        AuthStore::open(database.clone())
            .unwrap()
            .validate_session(&session.token)
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .rotate_role_password("initial-password", AuthRole::Admin, "updated-password")
            .unwrap()
    );
    store
        .bootstrap_password(AuthRole::Admin, "initial-password")
        .unwrap();
    assert!(
        store
            .authenticate_session("initial-password", 100)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .authenticate_session("updated-password", 100)
            .unwrap()
            .is_some()
    );
    assert!(store.validate_session(&session.token).unwrap().is_none());
    assert!(store.authenticate_session("updated-password", 0).is_err());
}
#[test]
fn concurrent_role_configuration_cannot_make_passwords_equal() {
    let store = Arc::new(AuthStore::open(database()).unwrap());
    let barrier = Arc::new(Barrier::new(2));
    let handles = [AuthRole::Admin, AuthRole::Chat].map(|role| {
        let store = store.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.set_password(role, "one-shared-password")
        })
    });
    let successes = handles
        .into_iter()
        .filter_map(|h| h.join().unwrap().ok())
        .count();
    assert_eq!(successes, 1);
}
#[test]
fn registration_directory_and_user_grants_persist_without_leaking_passwords() {
    let db = database();
    let store = AuthStore::open(db.clone()).unwrap();
    let code = store
        .issue_registration_code("user@example.com")
        .unwrap()
        .unwrap();
    store.activate_registration_code(&code.id).unwrap();
    let session = store
        .register_verified_user(
            &code.email,
            &code.code,
            "student",
            "同学",
            "account-password",
            100,
        )
        .unwrap();
    assert_eq!(
        store
            .user_agent_profile_id(&session.account.id)
            .unwrap()
            .as_deref(),
        Some("xiaolan")
    );
    let policy = UserCapabilityPolicy {
        install_skills: true,
        ..Default::default()
    };
    store
        .set_user_capability_policy(&session.account.id, policy)
        .unwrap();
    let reopened = AuthStore::open(db).unwrap();
    assert_eq!(
        reopened
            .user_capability_policy(&session.account.id)
            .unwrap(),
        Some(policy)
    );
    assert!(
        reopened
            .validate_user_session(&session.token)
            .unwrap()
            .is_some()
    );
    let (users, total) = reopened.user_directory("同学", None, 1).unwrap();
    assert_eq!(total, 1);
    assert_eq!(users.len(), 1);
    assert!(
        !serde_json::to_string(&users[0].0)
            .unwrap()
            .contains("account-password")
    );
    assert_eq!(reopened.user_directory("%", None, 100).unwrap().1, 0);
    assert!(
        !reopened
            .update_user_password(&session.account.id, "wrong-password", "next-password")
            .unwrap()
    );
    assert!(
        reopened
            .update_user_password(&session.account.id, "account-password", "next-password")
            .unwrap()
    );
    assert!(
        reopened
            .validate_user_session(&session.token)
            .unwrap()
            .is_none()
    );
    assert!(
        reopened
            .authenticate_user("user@example.com", "next-password", 100)
            .unwrap()
            .is_some()
    );
    assert!(
        reopened
            .register_verified_user(
                &code.email,
                &code.code,
                "other",
                "同学",
                "account-password",
                100
            )
            .is_err()
    );
}
#[test]
fn login_admission_is_persisted_and_bounded() {
    let db = database();
    let store = AuthStore::open(db.clone()).unwrap();
    for _ in 0..10 {
        store.reserve_login_attempt("bucket").unwrap();
    }
    let reopened = AuthStore::open(db).unwrap();
    assert!(matches!(
        reopened.reserve_login_attempt("bucket"),
        Err(exoharness_identity::AuthError::RateLimited { .. })
    ));
    reopened.reserve_login_attempt("other bucket").unwrap();
}
