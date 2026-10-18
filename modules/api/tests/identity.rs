#[path = "support/identity.rs"]
mod support;
use axum::http::StatusCode;
use exoharness_api::{identity_router, router};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use support::*;

#[tokio::test]
async fn verified_registration_profile_rotation_and_cross_surface_isolation() {
    let f = fixture().await;
    let (id, cookie, csrf) = register(&f).await;
    assert_eq!(
        request(
            &f.portal,
            9798,
            "GET",
            "/v1/account",
            Value::Null,
            Some(&cookie),
            None
        )
        .await
        .body["account"]["id"],
        id
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "GET",
            "/v1/management/users",
            Value::Null,
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &f.portal,
            9798,
            "PUT",
            "/v1/account/profile",
            json!({"display_name":"小朋友"}),
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let reply = request(
        &f.portal,
        9798,
        "PUT",
        "/v1/account/profile",
        json!({"display_name":"小朋友"}),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["account"]["display_name"], "小朋友");
    assert_eq!(
        request(
            &f.portal,
            9798,
            "PUT",
            "/v1/account/password",
            json!({"current_password":"wrong","new_password":"new-student-password"}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &f.portal,
            9798,
            "PUT",
            "/v1/account/password",
            json!({"current_password":"student-password","new_password":"new-student-password"}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &f.portal,
            9798,
            "GET",
            "/v1/auth/session",
            Value::Null,
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &f.portal,
            9798,
            "POST",
            "/v1/auth/login",
            json!({"identifier":"student@example.com","password":"new-student-password"}),
            None,
            None
        )
        .await
        .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn administrator_directory_grants_assignment_and_persisted_session() {
    let f = fixture().await;
    let (id, user_cookie, _) = register(&f).await;
    let (cookie, csrf) = admin_login(&f).await;
    let users = request(
        &f.admin,
        9797,
        "GET",
        "/v1/management/users?search=%E5%90%8C%E5%AD%A6",
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(users.status, StatusCode::OK);
    assert_eq!(users.body["total"], 1);
    assert_eq!(users.body["users"][0]["conversation_count"], 0);
    assert_eq!(
        users.body["users"][0]["capability_policy"]["install_mcp"],
        false
    );
    assert!(!users.body.to_string().contains("student-password"));
    assert!(
        !users
            .body
            .to_string()
            .contains("Private system instructions")
    );
    let policy = json!({"policy":{"install_skills":true,"install_mcp":true,"install_tools":false,"use_skills":true,"use_mcp":true,"use_tools":true}});
    let path = format!("/v1/management/users/{id}/capability-policy");
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            &path,
            policy.clone(),
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            &path,
            policy,
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .body["policy"]["install_mcp"],
        true
    );
    let path = format!("/v1/management/users/{id}/assistant");
    for id in ["inactive", "private", "missing"] {
        assert_eq!(
            request(
                &f.admin,
                9797,
                "PUT",
                &path,
                json!({"agent_profile_id":id}),
                Some(&cookie),
                Some(&csrf)
            )
            .await
            .status,
            StatusCode::BAD_REQUEST
        );
    }
    let reply = request(
        &f.admin,
        9797,
        "PUT",
        &path,
        json!({"agent_profile_id":"coder"}),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["assistant_profile_id"], "coder");
    let profile = request(
        &f.portal,
        9798,
        "GET",
        "/v1/agent-profiles/default",
        Value::Null,
        Some(&user_cookie),
        None,
    )
    .await;
    assert_eq!(profile.status, StatusCode::OK);
    assert_eq!(profile.body["id"], "coder");
    assert!(profile.body.get("system_prompt").is_none());
    assert!(profile.body.get("access").is_none());
    let portal_profile = request(
        &f.portal,
        9798,
        "GET",
        "/v1/agent-profile",
        Value::Null,
        Some(&user_cookie),
        None,
    )
    .await;
    assert_eq!(portal_profile.status, StatusCode::OK);
    assert_eq!(portal_profile.body, profile.body);
    assert_eq!(
        request(
            &f.portal,
            9798,
            "GET",
            "/v1/agent-profile",
            Value::Null,
            None,
            None,
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let rebuilt = router(f.admin_state.clone()).merge(identity_router(
        f.admin_state.clone(),
        f.store.clone(),
        f.mailer.clone(),
        f.assistants.clone(),
    ));
    assert_eq!(
        request(
            &rebuilt,
            9797,
            "GET",
            "/v1/auth/session",
            Value::Null,
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    let reply = request(
        &rebuilt,
        9797,
        "GET",
        &format!("/v1/management/users/{id}"),
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(reply.body["assistant_profile_id"], "coder");
    assert_eq!(reply.body["capability_policy"]["install_skills"], true);
    assert_eq!(
        request(
            &f.admin,
            9797,
            "GET",
            "/v1/management/users?search=%25",
            Value::Null,
            Some(&cookie),
            None
        )
        .await
        .body["total"],
        0
    );
}

#[tokio::test]
async fn management_readonly_password_and_email_template_are_enforced() {
    let f = fixture().await;
    let (cookie, csrf) = admin_login(&f).await;
    let template = request(
        &f.admin,
        9797,
        "GET",
        "/v1/auth/registration-email",
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(template.status, StatusCode::OK);
    let body = json!({"subject":"验证码","body":"你的验证码 {{code}}","revision":template.body["revision"]});
    let reply = request(
        &f.admin,
        9797,
        "PUT",
        "/v1/auth/registration-email",
        body.clone(),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            "/v1/auth/registration-email",
            body,
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            "/v1/auth/passwords/chat",
            json!({"current_password":ADMIN,"new_password":"readonly-password"}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::OK
    );
    let chat = request(
        &f.admin,
        9797,
        "POST",
        "/v1/auth/login",
        json!({"password":"readonly-password"}),
        None,
        None,
    )
    .await;
    assert_eq!(chat.status, StatusCode::OK);
    assert_eq!(chat.body["role"], "chat");
    let readonly = chat.cookie.unwrap();
    assert_eq!(
        request(
            &f.admin,
            9797,
            "GET",
            "/v1/auth/session",
            Value::Null,
            Some(&readonly),
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "GET",
            "/v1/management/users",
            Value::Null,
            Some(&readonly),
            None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            "/v1/auth/passwords/admin",
            json!({"current_password":ADMIN,"new_password":"updated-admin-password"}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "GET",
            "/v1/auth/session",
            Value::Null,
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "GET",
            "/v1/auth/session",
            Value::Null,
            Some(&readonly),
            None
        )
        .await
        .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn mail_failure_does_not_activate_code_and_registration_fields_are_native() {
    let f = fixture().await;
    f.mailer.fail.store(true, Ordering::Relaxed);
    let reply = request(
        &f.portal,
        9798,
        "POST",
        "/v1/auth/registration-code",
        json!({"email":"student@example.com"}),
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reply.body["resumable"], false);
    assert!(f.mailer.delivered.lock().unwrap().is_empty());
    assert_eq!(request(&f.portal,9798,"POST","/v1/auth/register",json!({"email":"student@example.com","code":"123456","username":"student","display_name":"同学","password":"student-password"}),None,None).await.status,StatusCode::BAD_REQUEST);
    assert_eq!(request(&f.portal,9798,"POST","/v1/auth/register",json!({"email":"student@example.com","code":123456,"username":"student","display_name":"同学","password":"student-password"}),None,None).await.status,StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn registered_email_is_explicit_without_sending_mail_or_changing_the_account() {
    let f = fixture().await;
    let (id, _, _) = register(&f).await;
    assert_eq!(f.mailer.delivered.lock().unwrap().len(), 1);
    for email in [
        "student@example.com",
        "STUDENT@EXAMPLE.COM",
        " student@example.com ",
    ] {
        let reply = request(
            &f.portal,
            9798,
            "POST",
            "/v1/auth/registration-code",
            json!({"email":email}),
            None,
            None,
        )
        .await;
        assert_eq!(reply.status, StatusCode::CONFLICT);
        assert_eq!(reply.body["error"]["code"], "email_already_registered");
        assert_eq!(reply.body["error"]["message"], "该邮箱已注册，请直接登录");
        assert!(reply.body.get("resend_after").is_none());
        assert_eq!(f.mailer.delivered.lock().unwrap().len(), 1);
    }
    let login = request(
        &f.portal,
        9798,
        "POST",
        "/v1/auth/login",
        json!({"identifier":"student@example.com","password":"student-password"}),
        None,
        None,
    )
    .await;
    assert_eq!(login.status, StatusCode::OK);
    assert_eq!(login.body["account"]["id"], id);
    let fresh = request(
        &f.portal,
        9798,
        "POST",
        "/v1/auth/registration-code",
        json!({"email":"new-person@example.com"}),
        None,
        None,
    )
    .await;
    assert_eq!(fresh.status, StatusCode::ACCEPTED);
    assert_eq!(fresh.body["message"], "验证码已发送，请查收邮箱");
    assert_eq!(fresh.body["resend_after"], 60);
    assert_eq!(f.mailer.delivered.lock().unwrap().len(), 2);
}
