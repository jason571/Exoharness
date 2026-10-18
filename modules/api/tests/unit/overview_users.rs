use super::*;
use identity_support::{Fixture, admin_login, request};

async fn register(f: &Fixture, username: &str, nickname: &str) -> String {
    let email = format!("{username}@example.com");
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
    assert_eq!(reply.status, StatusCode::ACCEPTED);
    let code = f
        .mailer
        .delivered
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(target, _)| target == &email)
        .unwrap()
        .1
        .clone();
    let reply = request(&f.portal, 9798, "POST", "/v1/auth/register", json!({"email":email,"code":code,"username":username,"display_name":nickname,"password":"fixture-account-password"}), None, None).await;
    assert_eq!(reply.status, StatusCode::CREATED);
    reply.body["account"]["id"].as_str().unwrap().to_owned()
}

async fn seed(f: &Fixture, owners: Vec<String>) {
    let store = f.admin_state.execution.0.clone();
    tokio::task::spawn_blocking(move || {
        for (index, owner) in owners.iter().enumerate() {
            let session = format!("overview-session-{index}");
            store.create_session(owner, &session).unwrap();
            store
                .create_turn(CreateTurn {
                    owner,
                    session_id: &session,
                    expected_session_version: 0,
                    turn_id: &format!("overview-turn-{index}"),
                    run_id: &format!("overview-run-{index}"),
                    input: "Overview identity regression",
                    snapshot: &ExecutionSnapshot {
                        version: 1,
                        assistant_id: "xiaolan".into(),
                        assistant_revision: 1,
                        model: ModelSnapshot {
                            config_id: "fixture".into(),
                            revision: 1,
                            adapter: "mock".into(),
                            settings: json!({}),
                        },
                        capabilities: vec![],
                        max_consecutive_identical_calls: 3,
                        requires_execution: false,
                    },
                })
                .unwrap();
        }
    })
    .await
    .unwrap();
}

async fn overview(router: &Router, cookie: &str, suffix: &str) -> Value {
    let reply = request(
        router,
        9797,
        "GET",
        &format!("/v1/management/overview{suffix}"),
        Value::Null,
        Some(cookie),
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    reply.body
}

fn owners(value: &Value) -> std::collections::BTreeSet<String> {
    value["groups"]["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn registered_names_are_projected_and_filtered_before_aggregation() {
    let f = identity_support::fixture().await;
    let flyang = register(&f, "flyang", "在水一方").await;
    let second = register(&f, "flyang_two", "在水一方").await;
    let bob = register(&f, "bob", "bob").await;
    let no_runs = register(&f, "no_runs", "没有运行").await;
    seed(
        &f,
        vec![
            flyang.clone(),
            second.clone(),
            bob.clone(),
            "admin".into(),
            "management".into(),
            "old-owner".into(),
        ],
    )
    .await;
    let router = management_router(f.admin_state.clone(), f.assistants.clone());
    let (cookie, _) = admin_login(&f).await;
    let value = overview(&router, &cookie, "").await;
    assert_eq!(value["runs"], 6);
    assert_eq!(value["user_labels"][&flyang], "在水一方 · flyang");
    assert_eq!(value["user_labels"][&second], "在水一方 · flyang_two");
    assert_eq!(value["user_labels"][&bob], "bob");
    assert!(value["user_labels"].get(&no_runs).is_none());
    assert!(owners(&value).contains(&flyang));
    assert!(
        value["activity"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["user_id"] == flyang)
    );
    let expected = [flyang.clone(), second.clone()].into_iter().collect();
    for suffix in ["?user=FLYANG", "?user=%E5%9C%A8%E6%B0%B4%E4%B8%80%E6%96%B9"] {
        let filtered = overview(&router, &cookie, suffix).await;
        assert_eq!(owners(&filtered), expected);
        assert_eq!(filtered["runs"], 2);
        assert_eq!(filtered["counts"]["queued"], 2);
        assert_eq!(filtered["activity"].as_array().unwrap().len(), 2);
    }
    let exact = overview(&router, &cookie, "?user=flyang_two").await;
    assert_eq!(owners(&exact), [second.clone()].into_iter().collect());
    let underscore = overview(&router, &cookie, "?user=_").await;
    assert_eq!(owners(&underscore), [second].into_iter().collect());
    let by_id = overview(&router, &cookie, &format!("?user={flyang}")).await;
    assert_eq!(owners(&by_id), [flyang.clone()].into_iter().collect());
    assert_eq!(
        owners(&overview(&router, &cookie, "?user=old-owner").await),
        ["old-owner".to_owned()].into_iter().collect()
    );
    assert_eq!(
        owners(&overview(&router, &cookie, "?user=adm").await),
        ["admin".to_owned()].into_iter().collect()
    );
    assert_eq!(
        owners(&overview(&router, &cookie, "?user=management").await),
        ["management".to_owned()].into_iter().collect()
    );
    for suffix in [
        "?user=does-not-exist",
        "?user=%25",
        "?user=%27%20OR%201%3D1--",
    ] {
        let empty = overview(&router, &cookie, suffix).await;
        assert_eq!(empty["runs"], 0);
        assert_eq!(empty["activity"], json!([]));
        assert_eq!(empty["user_labels"], json!({}));
    }
    assert_eq!(overview(&router, &cookie, "?user=%20%20").await["runs"], 6);
    let store = f.store.clone();
    let id = flyang.clone();
    tokio::task::spawn_blocking(move || store.update_user_profile(&id, "新的昵称").unwrap())
        .await
        .unwrap();
    let renamed = overview(&router, &cookie, "?user=flyang").await;
    assert_eq!(renamed["user_labels"][&flyang], "新的昵称 · flyang");
    let text = serde_json::to_string(&renamed).unwrap();
    assert!(
        !text.contains("example.com")
            && !text.contains("password_hash")
            && !text.contains("token_digest")
    );
}

#[tokio::test]
async fn account_labels_stay_within_the_management_access_boundary() {
    let f = identity_support::fixture().await;
    let owner = register(&f, "flyang", "在水一方").await;
    seed(&f, vec![owner]).await;
    let router = management_router(f.admin_state.clone(), f.assistants.clone());
    let user_login = request(
        &f.portal,
        9798,
        "POST",
        "/v1/auth/login",
        json!({"identifier":"flyang","password":"fixture-account-password"}),
        None,
        None,
    )
    .await;
    assert_eq!(user_login.status, StatusCode::OK);
    let blocked = request(
        &router,
        9797,
        "GET",
        "/v1/management/overview",
        Value::Null,
        user_login.cookie.as_deref(),
        None,
    )
    .await;
    assert!(matches!(
        blocked.status,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ));
    assert!(blocked.body.get("user_labels").is_none());
    assert_eq!(
        request(
            &f.portal,
            9798,
            "GET",
            "/v1/management/overview",
            Value::Null,
            user_login.cookie.as_deref(),
            None
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}
