use crate::{Database, helpers::*, types::*};

#[test]
fn user_capability_policy_is_private_default_and_persists() {
    let store = AuthStore::open(Database::test("user_capability_policy").unwrap()).unwrap();
    let connection = store.connection().unwrap();
    connection
        .execute(
            "INSERT INTO auth_users
                (id, username, normalized_username, email, normalized_email,
                 display_name, password_hash, revision, created_at, updated_at)
             VALUES ('user_test', 'test', 'test', 'test@example.com',
                     'test@example.com', 'Test', 'unused', 1, 1, 1)",
            (),
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO auth_users
                (id, username, normalized_username, email, normalized_email,
                 display_name, password_hash, revision, created_at, updated_at)
             VALUES ('user_other', 'other', 'other', 'other@example.com',
                     'other@example.com', 'Other', 'unused', 1, 1, 1)",
            (),
        )
        .unwrap();
    assert_eq!(store.user_capability_policy("missing").unwrap(), None);
    assert_eq!(
        store.user_capability_policy("user_test").unwrap(),
        Some(UserCapabilityPolicy::default())
    );
    let changed = UserCapabilityPolicy {
        install_skills: true,
        use_mcp: false,
        ..Default::default()
    };
    assert_eq!(
        store
            .set_user_capability_policy("user_test", changed)
            .unwrap(),
        changed
    );
    assert_eq!(
        store.user_capability_policy("user_test").unwrap(),
        Some(changed)
    );
    assert!(
        store
            .set_user_capability_policy("missing", changed)
            .is_err()
    );
    let grant = CapabilityShareGrant {
        revision: 2,
        manifest_digest: "manifest-a".into(),
        source_digest: Some("source-a".into()),
    };
    store
        .set_capability_share("skill:user:user_test:example", "user_other", &grant, true)
        .unwrap();
    assert_eq!(
        store
            .user_shared_capabilities("user_other")
            .unwrap()
            .get("skill:user:user_test:example"),
        Some(&grant)
    );
    assert_eq!(
        store
            .capability_shared_users("skill:user:user_test:example", &grant)
            .unwrap(),
        vec!["user_other"]
    );
    assert!(
        store
            .capability_shared_users(
                "skill:user:user_test:example",
                &CapabilityShareGrant {
                    revision: 2,
                    manifest_digest: "manifest-a".into(),
                    source_digest: Some("source-b".into()),
                }
            )
            .unwrap()
            .is_empty()
    );
    store
        .set_capability_share("skill:user:user_test:example", "user_other", &grant, false)
        .unwrap();
    assert!(
        store
            .user_shared_capabilities("user_other")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn passwords_are_hashed_and_sessions_are_invalidated_on_rotation() {
    let store = AuthStore::open(Database::test("auth_passwords").unwrap()).unwrap();
    store
        .set_password(AuthRole::Admin, "correct horse battery staple")
        .unwrap();
    assert_eq!(
        store.authenticate("correct horse battery staple").unwrap(),
        Some(AuthRole::Admin)
    );
    let session = store
        .create_session(AuthRole::Admin, DEFAULT_SESSION_TTL_SECONDS)
        .unwrap();
    assert!(store.validate_session(&session.token).unwrap().is_some());
    store
        .set_password(AuthRole::Admin, "a different correct password")
        .unwrap();
    assert!(store.validate_session(&session.token).unwrap().is_none());
}

#[test]
fn roles_cannot_share_a_password() {
    let store = AuthStore::open(Database::test("auth_roles").unwrap()).unwrap();
    store
        .set_password(AuthRole::Admin, "correct horse battery staple")
        .unwrap();
    let error = store
        .set_password(AuthRole::Chat, "correct horse battery staple")
        .unwrap_err();
    assert!(matches!(error, AuthError::Validation(_)));
}

#[test]
fn passwords_accept_eight_characters_but_reject_shorter_values() {
    assert!(validate_password("abcd1234").is_ok());
    assert!(matches!(
        validate_password("abc1234"),
        Err(AuthError::Validation(_))
    ));
}

#[test]
fn registration_email_template_is_validated_versioned_and_rendered() {
    let store = AuthStore::open(Database::test("auth_email_template").unwrap()).unwrap();
    let default_template = store.registration_email_template().unwrap();
    assert_eq!(default_template.revision, 0);
    let (default_subject, default_body) = default_template.render("123456");
    assert_eq!(default_subject, "ExoHarness 注册验证码");
    assert!(default_body.contains("123456"));
    assert!(default_body.contains("10 分钟"));

    let updated = store
        .update_registration_email_template(
            "{{product_name}}｜欢迎注册",
            "验证码：{{code}}\n请在 {{expires_minutes}} 分钟内完成注册。",
            0,
        )
        .unwrap();
    assert_eq!(updated.revision, 1);
    let persisted = store.registration_email_template().unwrap();
    assert_eq!(persisted, updated);
    let (subject, body) = persisted.render("654321");
    assert_eq!(subject, "ExoHarness｜欢迎注册");
    assert_eq!(body, "验证码：654321\n请在 10 分钟内完成注册。");

    assert!(matches!(
        store.update_registration_email_template("标题", "缺少验证码", 1),
        Err(AuthError::Validation(_))
    ));
    assert!(matches!(
        store.update_registration_email_template("标题", "{{unknown}} {{code}}", 1),
        Err(AuthError::Validation(_))
    ));
    assert!(matches!(
        store.update_registration_email_template("标题", "验证码 {{code}}", 0),
        Err(AuthError::Conflict(_))
    ));
}

#[test]
fn public_registration_requires_an_activated_email_code() {
    let store = AuthStore::open(Database::test("auth_registration").unwrap()).unwrap();
    let issued = store
        .issue_registration_code("person@example.com")
        .unwrap()
        .unwrap();
    assert!(matches!(
        store.register_verified_user(
            "person@example.com",
            &issued.code,
            "person",
            "Person",
            "correct-password",
            DEFAULT_SESSION_TTL_SECONDS,
        ),
        Err(AuthError::Validation(_))
    ));
    store.activate_registration_code(&issued.id).unwrap();
    assert!(matches!(
        store.register_verified_user(
            "person@example.com",
            "000000",
            "person",
            "Person",
            "correct-password",
            DEFAULT_SESSION_TTL_SECONDS,
        ),
        Err(AuthError::Validation(_))
    ));
    let session = store
        .register_verified_user(
            "person@example.com",
            &issued.code,
            "person",
            "Person",
            "correct-password",
            DEFAULT_SESSION_TTL_SECONDS,
        )
        .unwrap();
    assert_eq!(session.account.email, "person@example.com");
    assert!(
        store
            .validate_user_session(&session.token)
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .authenticate_user(
                "PERSON@EXAMPLE.COM",
                "correct-password",
                DEFAULT_SESSION_TTL_SECONDS,
            )
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .issue_registration_code("person@example.com")
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        store.register_verified_user(
            "person@example.com",
            &issued.code,
            "person2",
            "Person",
            "correct-password",
            DEFAULT_SESSION_TTL_SECONDS,
        ),
        Err(AuthError::Validation(_))
    ));
}
