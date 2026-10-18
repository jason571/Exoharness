use exoharness_identity::{AuthStore, Database, external::ApplicationInput};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
fn fixture() -> AuthStore {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    AuthStore::open(
        Database::connect(
            &std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").unwrap(),
            None,
            &format!(
                "pro_backend_{}_{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ),
        )
        .unwrap(),
    )
    .unwrap()
}
fn input(id: &str, require: bool) -> ApplicationInput {
    serde_json::from_value(json!({"client_id":id,"name":id,"assistant_id":"role","scopes":["identity:read","sessions:read","sessions:write"],"require_device_id":require})).unwrap()
}
#[test]
fn backend_keys_map_users_and_devices_without_browser_login_or_cross_application_impersonation() {
    let s = fixture();
    let a = s.create_external_application(input("care", true)).unwrap();
    let b = s
        .create_external_application(input("other", false))
        .unwrap();
    assert!(
        s.external_backend_grant("care", &a.client_secret, "one", Some("flyang"), None)
            .is_err()
    );
    assert!(
        s.external_backend_grant(
            "care",
            &b.client_secret,
            "one",
            Some("flyang"),
            Some("watch-a")
        )
        .unwrap()
        .is_none()
    );
    let ga = s
        .external_backend_grant(
            "care",
            &a.client_secret,
            "one",
            Some("flyang"),
            Some("watch-a"),
        )
        .unwrap()
        .unwrap();
    let again = s
        .external_backend_grant(
            "care",
            &a.client_secret,
            "one",
            Some("flyang"),
            Some("watch-a"),
        )
        .unwrap()
        .unwrap();
    assert_eq!(ga.id, again.id);
    assert_eq!(ga.user_id, again.user_id);
    let watch_b = s
        .external_backend_grant(
            "care",
            &a.client_secret,
            "one",
            Some("flyang"),
            Some("watch-b"),
        )
        .unwrap()
        .unwrap();
    assert_eq!(ga.user_id, watch_b.user_id);
    assert_ne!(ga.id, watch_b.id);
    let other_user = s
        .external_backend_grant(
            "care",
            &a.client_secret,
            "two",
            Some("Second"),
            Some("watch-a"),
        )
        .unwrap()
        .unwrap();
    assert_ne!(ga.user_id, other_user.user_id);
    let other_app = s
        .external_backend_grant("other", &b.client_secret, "one", Some("flyang"), None)
        .unwrap()
        .unwrap();
    assert_ne!(ga.user_id, other_app.user_id);
    assert!(!other_app.require_device_id);
    assert_eq!(
        s.user_account(&ga.user_id).unwrap().unwrap().display_name,
        "flyang"
    );
    assert!(
        s.authenticate_user("flyang", "!external-application", 300)
            .unwrap()
            .is_none()
    );
    let tokens = s
        .external_backend_tokens("care", &a.client_secret, "one", None, Some("watch-a"))
        .unwrap()
        .unwrap();
    let access = s
        .validate_external_access(&tokens.access_token)
        .unwrap()
        .unwrap();
    assert_eq!(access.user_id, ga.user_id);
    assert_eq!(access.device_id.as_deref(), Some("watch-a"));
    let changed = s
        .create_external_application(input("withheld", false))
        .unwrap();
    let mut config = changed.application.input.clone();
    config.allow_backend_identity = false;
    config.redirect_uris = vec!["https://app.example/callback".into()];
    s.update_external_application(config, 1, false).unwrap();
    assert!(
        s.external_backend_grant("withheld", &changed.client_secret, "one", None, None)
            .unwrap()
            .is_none()
    );
}
#[test]
fn revocation_and_rotation_are_not_bypassed_by_repeated_backend_assertions() {
    let s = fixture();
    let app = s.create_external_application(input("care", true)).unwrap();
    let grant = s
        .external_backend_grant("care", &app.client_secret, "user", None, Some("watch"))
        .unwrap()
        .unwrap();
    s.revoke_external_application_grant("care", &grant.id)
        .unwrap();
    assert!(
        s.external_backend_grant("care", &app.client_secret, "user", None, Some("watch"))
            .is_err()
    );
    let next = s
        .update_external_application(app.application.input.clone(), 1, true)
        .unwrap();
    assert!(
        s.external_backend_grant("care", &app.client_secret, "user", None, Some("watch"))
            .unwrap()
            .is_none()
    );
    let renewed = s
        .external_backend_grant("care", &next.client_secret, "user", None, Some("watch"))
        .unwrap()
        .unwrap();
    assert_eq!(grant.user_id, renewed.user_id);
    assert_ne!(grant.id, renewed.id);
    let mut config = next.application.input.clone();
    config.enabled = false;
    s.update_external_application(config, 2, false).unwrap();
    assert!(
        s.external_backend_grant("care", &next.client_secret, "user", None, Some("watch"))
            .unwrap()
            .is_none()
    );
}
