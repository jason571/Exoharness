#[path = "support/identity.rs"]
#[allow(dead_code)]
mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{ApiState, Surface, identity_router_with_public_url};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use support::*;
use tower::ServiceExt;
fn token(link: &str) -> String {
    let link = url::Url::parse(link).unwrap();
    assert_eq!(link.host_str(), Some("agent.example"));
    assert_eq!(link.path(), "/reset-password");
    assert!(link.query().is_none());
    url::form_urlencoded::parse(link.fragment().unwrap().as_bytes())
        .find(|(key, _)| key == "token")
        .unwrap()
        .1
        .into_owned()
}
fn routes(f: &Fixture, base: &str) -> axum::Router {
    identity_router_with_public_url(
        ApiState {
            surface: Surface::Portal,
            port: 9798,
            ..f.admin_state.clone()
        },
        f.store.clone(),
        f.mailer.clone(),
        f.assistants.clone(),
        None,
        base,
    )
}
#[tokio::test]
async fn request_email_link_complete_reset_revokes_old_login_and_rejects_replay() {
    let f = fixture().await;
    let (_, cookie, _) = register(&f).await;
    let app = routes(&f, "https://agent.example/");
    let sent = request(
        &app,
        9798,
        "POST",
        "/v1/auth/password-reset",
        json!({"email":"student@example.com"}),
        None,
        None,
    )
    .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED);
    assert_eq!(sent.body["expires_in"], 1200);
    assert!(sent.body.get("token").is_none());
    let reset = token(&f.mailer.reset_links.lock().unwrap()[0].1);
    let validation = request(
        &app,
        9798,
        "POST",
        "/v1/auth/password-reset/validate",
        json!({"token":reset}),
        None,
        None,
    )
    .await;
    assert_eq!(validation.status, StatusCode::OK);
    assert_eq!(validation.body["valid"], true);
    let completed = request(
        &app,
        9798,
        "POST",
        "/v1/auth/password-reset/complete",
        json!({"token":reset,"new_password":"updated-password"}),
        None,
        None,
    )
    .await;
    assert_eq!(completed.status, StatusCode::OK);
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
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &f.portal,
            9798,
            "POST",
            "/v1/auth/login",
            json!({"identifier":"student","password":"student-password"}),
            None,
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
            json!({"identifier":"student","password":"updated-password"}),
            None,
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    let replay = request(
        &app,
        9798,
        "POST",
        "/v1/auth/password-reset/complete",
        json!({"token":reset,"new_password":"other-password"}),
        None,
        None,
    )
    .await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST);
    assert_eq!(replay.body["error"]["code"], "invalid_password_reset");
}
#[tokio::test]
async fn unknown_email_delivery_failure_and_throttling_are_explicit_without_false_success() {
    let f = fixture().await;
    register(&f).await;
    let app = routes(&f, "https://agent.example");
    assert_eq!(
        request(
            &app,
            9798,
            "POST",
            "/v1/auth/password-reset",
            json!({"email":"unknown@example.com"}),
            None,
            None
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    f.mailer.fail.store(true, Ordering::Relaxed);
    let failure = request(
        &app,
        9798,
        "POST",
        "/v1/auth/password-reset",
        json!({"email":"student@example.com"}),
        None,
        None,
    )
    .await;
    assert_eq!(failure.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(f.mailer.reset_links.lock().unwrap().is_empty());
    f.mailer.fail.store(false, Ordering::Relaxed);
    assert_eq!(
        request(
            &app,
            9798,
            "POST",
            "/v1/auth/password-reset",
            json!({"email":"student@example.com"}),
            None,
            None
        )
        .await
        .status,
        StatusCode::ACCEPTED
    );
    assert_eq!(
        request(
            &app,
            9798,
            "POST",
            "/v1/auth/password-reset",
            json!({"email":"student@example.com"}),
            None,
            None
        )
        .await
        .status,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(f.mailer.reset_links.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn invalid_public_url_and_forged_token_fail_before_any_password_change() {
    let f = fixture().await;
    register(&f).await;
    for base in [
        "http://evil.example/",
        "https://user:password@agent.example/",
        "https://agent.example/?redirect=evil",
        "https://agent.example/#fragment",
    ] {
        let app = routes(&f, base);
        assert_eq!(
            request(
                &app,
                9798,
                "POST",
                "/v1/auth/password-reset",
                json!({"email":"student@example.com"}),
                None,
                None
            )
            .await
            .status,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    let app = routes(&f, "https://agent.example");
    let invalid = request(
        &app,
        9798,
        "POST",
        "/v1/auth/password-reset/validate",
        json!({"token":"forged"}),
        None,
        None,
    )
    .await;
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid.body["error"]["code"], "invalid_password_reset");
    assert!(f.mailer.reset_links.lock().unwrap().is_empty());
}

#[tokio::test]
async fn recovery_pages_are_portal_only_and_cross_origin_writes_are_rejected() {
    let f = fixture().await;
    for (path, marker) in [
        ("/forgot-password", "forgot-password-form"),
        ("/reset-password", "reset-password-form"),
    ] {
        let response = f
            .portal
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        assert!(std::str::from_utf8(&body).unwrap().contains(marker));
        assert_eq!(
            f.admin
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    for path in [
        "/v1/auth/password-reset",
        "/v1/auth/password-reset/validate",
        "/v1/auth/password-reset/complete",
    ] {
        let response = f.portal.clone().oneshot(Request::builder()
            .method("POST").uri(path).header("host", "127.0.0.1:9798")
            .header("origin", "https://untrusted.example")
            .header("content-type", "application/json")
            .body(Body::from(json!({"email":"person@example.com","token":"forged","new_password":"new-password"}).to_string())).unwrap()).await.unwrap();
        // Unknown fields are rejected before the handler; a correctly shaped hostile request is checked below.
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let input = match path {
            "/v1/auth/password-reset" => json!({"email":"person@example.com"}),
            "/v1/auth/password-reset/validate" => json!({"token":"forged"}),
            _ => json!({"token":"forged","new_password":"new-password"}),
        };
        let response = f
            .portal
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("host", "127.0.0.1:9798")
                    .header("origin", "https://untrusted.example")
                    .header("content-type", "application/json")
                    .body(Body::from(input.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            request(&f.admin, 9797, "POST", path, input, None, None)
                .await
                .status,
            StatusCode::NOT_FOUND
        );
    }
    assert!(f.mailer.reset_links.lock().unwrap().is_empty());
}

#[tokio::test]
async fn weak_password_keeps_link_valid_and_does_not_revoke_current_login() {
    let f = fixture().await;
    let (_, cookie, _) = register(&f).await;
    let app = routes(&f, "https://agent.example");
    assert_eq!(
        request(
            &app,
            9798,
            "POST",
            "/v1/auth/password-reset",
            json!({"email":"  STUDENT@EXAMPLE.COM  "}),
            None,
            None
        )
        .await
        .status,
        StatusCode::ACCEPTED
    );
    let reset = token(&f.mailer.reset_links.lock().unwrap()[0].1);
    assert_eq!(
        request(
            &app,
            9798,
            "POST",
            "/v1/auth/password-reset/complete",
            json!({"token":reset,"new_password":"short"}),
            None,
            None
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &app,
            9798,
            "POST",
            "/v1/auth/password-reset/validate",
            json!({"token":reset}),
            None,
            None
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
            "/v1/account",
            Value::Null,
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            9798,
            "POST",
            "/v1/auth/password-reset/complete",
            json!({"token":reset,"new_password":"valid-new-password"}),
            None,
            None
        )
        .await
        .status,
        StatusCode::OK
    );
}
