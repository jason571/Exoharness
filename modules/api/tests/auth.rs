use exoharness_api::auth::{Credentials, Role};

#[test]
fn local_admin_password_requires_explicit_kind_and_does_not_weaken_api_keys() {
    let auth = Credentials::from_json(
        r#"[{"id":"admin","role":"admin","kind":"local_password","token":"test-password-11"}]"#,
    )
    .unwrap();
    assert_eq!(
        auth.authenticate("test-password-11").unwrap().role,
        Role::Admin
    );
    assert!(auth.authenticate("wrong").is_none());
    for invalid in [
        r#"[{"id":"admin","role":"admin","token":"test-password-11"}]"#,
        r#"[{"id":"user","role":"user","kind":"local_password","token":"test-password-11"}]"#,
        r#"[{"id":"admin","role":"admin","kind":"local_password","token":"short"}]"#,
        r#"[{"id":"admin","role":"admin","kind":"unknown","token":"test-password-11"}]"#,
    ] {
        assert!(Credentials::from_json(invalid).is_err());
    }
}

#[test]
fn keys_have_distinct_scoped_principals() {
    let auth=Credentials::from_json(r#"[{"id":"admin","role":"admin","token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},{"id":"one","role":"user","token":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}]"#).unwrap();
    assert_eq!(
        auth.authenticate("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap()
            .role,
        Role::Admin
    );
    assert_eq!(
        auth.authenticate("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
            .unwrap()
            .id,
        "one"
    );
    assert!(auth.authenticate("wrong").is_none());
}

#[test]
fn empty_short_duplicate_and_unknown_credential_fields_are_rejected() {
    for json in [
        "[]",
        r#"[{"id":"a","role":"user","token":"short"}]"#,
        r#"[{"id":"a","role":"user","token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},{"id":"a","role":"admin","token":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}]"#,
        r#"[{"id":"a","role":"user","token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},{"id":"b","role":"admin","token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]"#,
        r#"[{"id":"a","role":"user","token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","unsafe":true}]"#,
    ] {
        assert!(Credentials::from_json(json).is_err());
    }
}
