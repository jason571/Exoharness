use super::*;
use crate::{Database, UserAuthSession};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicU64, Ordering},
};
fn fixture() -> (AuthStore, Database, UserAuthSession) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let db = Database::test(&format!(
        "reset_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
    .unwrap();
    let store = AuthStore::open(db.clone()).unwrap();
    let code = store
        .issue_registration_code("person@example.com")
        .unwrap()
        .unwrap();
    store.activate_registration_code(&code.id).unwrap();
    let session = store
        .register_verified_user(
            &code.email,
            &code.code,
            "person",
            "Person",
            "original-password",
            3600,
        )
        .unwrap();
    (store, db, session)
}
fn active(store: &AuthStore) -> IssuedPasswordReset {
    let reset = store
        .issue_password_reset("PERSON@EXAMPLE.COM")
        .unwrap()
        .unwrap();
    store.activate_password_reset(&reset.id).unwrap();
    reset
}
#[test]
fn reset_is_inactive_until_mail_receipt_then_valid_for_exactly_twenty_minutes() {
    let (store, db, _) = fixture();
    let reset = store
        .issue_password_reset("person@example.com")
        .unwrap()
        .unwrap();
    assert!(store.password_reset_valid_for(&reset.token).is_err());
    assert!(
        store
            .complete_password_reset(&reset.token, "updated-password")
            .is_err()
    );
    store.activate_password_reset(&reset.id).unwrap();
    let (stored, ttl): (String, i64) = db
        .connection()
        .unwrap()
        .query_row(
            "SELECT token_digest,expires_at-activated_at FROM auth_password_resets WHERE id=?1",
            [reset.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_ne!(stored, reset.token);
    assert_eq!(stored.len(), 64);
    assert_eq!(ttl, 1200);
    assert!((1195..=1200).contains(&store.password_reset_valid_for(&reset.token).unwrap()));
    assert!(
        AuthStore::open(db)
            .unwrap()
            .password_reset_valid_for(&reset.token)
            .is_ok()
    );
}
#[test]
fn reset_is_single_use_changes_only_its_user_and_revokes_existing_sessions() {
    let (store, _, session) = fixture();
    let reset = active(&store);
    store
        .bootstrap_password(crate::AuthRole::Admin, "admin-password")
        .unwrap();
    let admin = store
        .authenticate_session("admin-password", 3600)
        .unwrap()
        .unwrap();
    store
        .complete_password_reset(&reset.token, "updated-password")
        .unwrap();
    assert!(
        store
            .validate_user_session(&session.token)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .authenticate_user("person", "original-password", 3600)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .authenticate_user("person", "updated-password", 3600)
            .unwrap()
            .is_some()
    );
    assert!(store.validate_session(&admin.token).unwrap().is_some());
    assert!(
        store
            .complete_password_reset(&reset.token, "another-password")
            .is_err()
    );
}
#[test]
fn expired_and_forged_links_cannot_change_a_password() {
    let (store, db, _) = fixture();
    let reset = active(&store);
    for token in ["", "../invalid", &"A".repeat(64)] {
        assert!(
            store
                .complete_password_reset(token, "bad-new-password")
                .is_err()
        );
    }
    db.connection()
        .unwrap()
        .execute(
            "UPDATE auth_password_resets SET expires_at=?1",
            [now_seconds()],
        )
        .unwrap();
    assert!(store.password_reset_valid_for(&reset.token).is_err());
    assert!(
        store
            .complete_password_reset(&reset.token, "bad-new-password")
            .is_err()
    );
    assert!(
        store
            .authenticate_user("person", "original-password", 3600)
            .unwrap()
            .is_some()
    );
}
#[test]
fn concurrency_allows_only_one_completion_and_no_second_password_overwrite() {
    let (store, _, _) = fixture();
    let reset = active(&store);
    let store = Arc::new(store);
    let barrier = Arc::new(Barrier::new(2));
    let workers = ["one-new-password", "two-new-password"].map(|password| {
        let (store, token, barrier) = (store.clone(), reset.token.clone(), barrier.clone());
        std::thread::spawn(move || {
            barrier.wait();
            store.complete_password_reset(&token, password).is_ok()
        })
    });
    assert_eq!(
        workers
            .into_iter()
            .filter_map(|worker| worker.join().ok())
            .filter(|ok| *ok)
            .count(),
        1
    );
    assert!(store.password_reset_valid_for(&reset.token).is_err());
}
#[test]
fn failed_delivery_keeps_old_link_and_new_delivered_link_supersedes_it() {
    let (store, db, _) = fixture();
    let first = active(&store);
    db.connection()
        .unwrap()
        .execute(
            "UPDATE auth_password_resets SET created_at=?1,activated_at=?1",
            [now_seconds() - 61],
        )
        .unwrap();
    let failed = store
        .issue_password_reset("person@example.com")
        .unwrap()
        .unwrap();
    store.discard_password_reset(&failed.id).unwrap();
    assert!(store.password_reset_valid_for(&first.token).is_ok());
    assert!(store.password_reset_valid_for(&failed.token).is_err());
    let second = active(&store);
    assert!(store.password_reset_valid_for(&first.token).is_err());
    assert!(store.password_reset_valid_for(&second.token).is_ok());
}
#[test]
fn normal_password_change_invalidates_previously_issued_links() {
    let (store, _, session) = fixture();
    let reset = active(&store);
    store
        .update_user_password(
            &session.account.id,
            "original-password",
            "normal-new-password",
        )
        .unwrap();
    assert!(
        store
            .complete_password_reset(&reset.token, "reset-new-password")
            .is_err()
    );
    assert!(
        store
            .authenticate_user("person", "normal-new-password", 3600)
            .unwrap()
            .is_some()
    );
}
#[test]
fn unknown_email_and_resend_limits_never_change_login_credentials() {
    let (store, db, _) = fixture();
    assert!(
        store
            .issue_password_reset("unknown@example.com")
            .unwrap()
            .is_none()
    );
    let first = active(&store);
    assert!(matches!(
        store.issue_password_reset("person@example.com"),
        Err(AuthError::RateLimited { .. })
    ));
    for _ in 1..5 {
        db.connection()
            .unwrap()
            .execute(
                "UPDATE auth_password_resets SET created_at=?1,activated_at=?1",
                [now_seconds() - 61],
            )
            .unwrap();
        active(&store);
    }
    db.connection()
        .unwrap()
        .execute(
            "UPDATE auth_password_resets SET created_at=?1,activated_at=?1",
            [now_seconds() - 61],
        )
        .unwrap();
    assert!(matches!(
        store.issue_password_reset("person@example.com"),
        Err(AuthError::RateLimited { .. })
    ));
    assert!(store.complete_password_reset(&first.token, "weak").is_err());
    assert!(
        store
            .authenticate_user("person", "original-password", 3600)
            .unwrap()
            .is_some()
    );
}
