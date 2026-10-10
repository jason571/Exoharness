#[path = "support/identity.rs"]
#[allow(dead_code)]
mod support;
use axum::http::StatusCode;
use exoharness_configuration::Assistants;
use serde_json::{Value, json};
use support::{admin_login, fixture, register, request};
fn input(id: &str) -> Value {
    json!({"id":id,"display_name":"Test assistant","avatar_text":"T","avatar":null,"description":"Fixture","system_prompt":"Private fixture instructions","skill_ids":[],"access":{"skills":{"mode":"none"},"mcp":{"mode":"none"},"tools":{"mode":"none"},"models":{"mode":"none"},"published":true},"enabled":true,"make_default":false,"actor":"spoofed-actor"})
}

#[tokio::test]
async fn imported_unavailable_references_can_be_preserved_without_granting_new_ones() {
    let f = fixture().await;
    let (cookie, csrf) = admin_login(&f).await;
    let mut value = input("imported-profile");
    for field in ["id", "enabled", "make_default", "actor"] {
        value.as_object_mut().unwrap().remove(field);
    }
    value["access"]["mcp"] = json!({"mode":"selected","ids":["mcp_server:global:_:not-installed"]});
    f.assistants
        .create("imported-profile", value.clone(), true, false)
        .unwrap();
    value["expected_revision"] = json!(1);
    value["access"]["tools"] = json!({"mode":"selected","ids":[exoharness_tools::SANDBOX_ID]});
    let response = request(
        &f.admin,
        9797,
        "PUT",
        "/v1/agent-profiles/imported-profile",
        value.clone(),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.body["access"]["mcp"]["ids"],
        json!(["mcp_server:global:_:not-installed"])
    );
    value["expected_revision"] = json!(2);
    value["access"]["mcp"]["ids"] = json!([
        "mcp_server:global:_:not-installed",
        "mcp_server:global:_:new-unregistered"
    ]);
    let response = request(
        &f.admin,
        9797,
        "PUT",
        "/v1/agent-profiles/imported-profile",
        value,
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_ne!(response.status, StatusCode::OK);
    assert_eq!(
        f.assistants.detail("imported-profile").unwrap()["active_revision"],
        2
    );
}

#[tokio::test]
async fn administrator_can_explicitly_grant_native_sandbox_but_not_unknown_builtin_ids() {
    let f = fixture().await;
    let (cookie, csrf) = admin_login(&f).await;
    let mut value = input("native-grant");
    value["access"]["tools"] = json!({"mode":"selected","ids":[exoharness_tools::SANDBOX_ID]});
    let response = request(
        &f.admin,
        9797,
        "POST",
        "/v1/agent-profiles",
        value.clone(),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(response.status, StatusCode::CREATED);
    assert_eq!(
        response.body["access"]["tools"]["ids"],
        json!([exoharness_tools::SANDBOX_ID])
    );
    value["id"] = json!("unknown-native-grant");
    value["access"]["tools"]["ids"] = json!(["tool:native:unknown"]);
    let rejected = request(
        &f.admin,
        9797,
        "POST",
        "/v1/agent-profiles",
        value,
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_ne!(rejected.status, StatusCode::CREATED);
    assert!(f.assistants.detail("unknown-native-grant").is_none());
}
#[tokio::test]
async fn original_assistant_creation_avatar_update_assignment_and_deletion_work() {
    let f = fixture().await;
    let (cookie, csrf) = admin_login(&f).await;
    let created = request(
        &f.admin,
        9797,
        "POST",
        "/v1/agent-profiles",
        input("fixture"),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED);
    assert_eq!(created.body["active_revision"], 1);
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let png = b"\x89PNG\r\n\x1a\nfixture";
    let upload=request(&f.admin,9797,"POST","/v1/agent-assets",json!({"content_type":"image/png","content_base64":STANDARD.encode(png),"actor":"web-console"}),Some(&cookie),Some(&csrf)).await;
    assert_eq!(upload.status, StatusCode::CREATED);
    let mut update = input("fixture");
    for field in ["id", "enabled", "make_default"] {
        update.as_object_mut().unwrap().remove(field);
    }
    update["expected_revision"] = json!(1);
    update["avatar"] = upload.body.clone();
    let changed = request(
        &f.admin,
        9797,
        "PUT",
        "/v1/agent-profiles/fixture",
        update.clone(),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(changed.status, StatusCode::OK);
    assert_eq!(changed.body["id"], "fixture");
    assert_eq!(changed.body["active_revision"], 2);
    assert_eq!(f.assistants.list().len(), 5);
    assert_eq!(
        Assistants::open(f._root.path())
            .unwrap()
            .detail("fixture")
            .unwrap()["avatar"],
        upload.body
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            "/v1/agent-profiles/fixture",
            update,
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    let (user, portal, _) = register(&f).await;
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            &format!("/v1/management/users/{user}/assistant"),
            json!({"agent_profile_id":"fixture"}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::OK
    );
    let public = request(
        &f.portal,
        9798,
        "GET",
        "/v1/agent-profiles/default",
        Value::Null,
        Some(&portal),
        None,
    )
    .await;
    assert_eq!(public.body["id"], "fixture");
    assert_eq!(public.body["avatar"], upload.body);
    for field in ["system_prompt", "access", "skill_ids"] {
        assert!(public.body.get(field).is_none());
    }
    assert_eq!(
        request(
            &f.admin,
            9797,
            "DELETE",
            "/v1/agent-profiles/fixture",
            json!({"confirm":true}),
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
            &format!("/v1/management/users/{user}/assistant"),
            json!({"agent_profile_id":"xiaolan"}),
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
            "DELETE",
            "/v1/agent-profiles/fixture",
            json!({"confirm":true}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert!(f.assistants.detail("fixture").is_none());
    assert_eq!(
        request(
            &f.portal,
            9798,
            "GET",
            &format!(
                "/v1/agent-assets/{}",
                upload.body["asset_id"].as_str().unwrap()
            ),
            Value::Null,
            Some(&portal),
            None
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn writes_enforce_csrf_scope_native_types_confirmation_and_default_rules() {
    let f = fixture().await;
    let (cookie, csrf) = admin_login(&f).await;
    let original = f.assistants.list();
    assert_eq!(
        request(
            &f.admin,
            9797,
            "POST",
            "/v1/agent-profiles",
            input("blocked"),
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let mut missing = input("blocked");
    missing["access"]["mcp"] = json!({"mode":"selected","ids":["mcp_server:global:_:absent"]});
    assert_eq!(
        request(
            &f.admin,
            9797,
            "POST",
            "/v1/agent-profiles",
            missing,
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    let mut malformed = input("blocked");
    malformed["enabled"] = json!("true");
    let failed = request(
        &f.admin,
        9797,
        "POST",
        "/v1/agent-profiles",
        malformed,
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(failed.status, StatusCode::BAD_REQUEST);
    assert_eq!(failed.body["resumable"], false);
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            "/v1/agent-profiles/xiaolan/enabled",
            json!({"enabled":false}),
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
            "DELETE",
            "/v1/agent-profiles/coder",
            json!({"confirm":false}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            "/v1/agent-profiles/inactive/default",
            json!({"confirm":true}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(f.assistants.list(), original);
    let (id, portal, portal_csrf) = register(&f).await;
    assert_eq!(
        request(
            &f.admin,
            9797,
            "POST",
            "/v1/agent-profiles",
            input("no-grant"),
            Some(&portal),
            Some(&portal_csrf)
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
            "/v1/agent-profiles",
            input("no-grant"),
            Some(&portal),
            Some(&portal_csrf)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    for assistant in ["inactive", "private"] {
        assert_eq!(
            request(
                &f.admin,
                9797,
                "PUT",
                &format!("/v1/management/users/{id}/assistant"),
                json!({"agent_profile_id":assistant}),
                Some(&cookie),
                Some(&csrf)
            )
            .await
            .status,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        request(
            &f.admin,
            9797,
            "PUT",
            "/v1/agent-profiles/coder/enabled",
            json!({"enabled":false}),
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
            "PUT",
            &format!("/v1/management/users/{id}/assistant"),
            json!({"agent_profile_id":"coder"}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
}
