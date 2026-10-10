use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{
    ApiState, ExecutionService, Surface, auth::Credentials, configuration_router, identity_router,
    router,
};
use exoharness_configuration::{Assistants, Models};
use exoharness_identity::{
    AuthStore, Database,
    mailer::{MailerError, RegistrationEmailMessage, RegistrationMailer},
};
use exoharness_storage_postgres::PostgresStore;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tokio::sync::watch;
use tower::ServiceExt;
pub const ADMIN: &str = "initial-admin-password";
pub struct Mailer {
    pub delivered: Mutex<Vec<(String, String)>>,
    pub fail: AtomicBool,
}
impl RegistrationMailer for Mailer {
    fn is_configured(&self) -> bool {
        true
    }
    fn send_registration_code(
        &self,
        email: &str,
        code: &str,
        _message: &RegistrationEmailMessage,
    ) -> Result<(), MailerError> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(MailerError::Send);
        }
        self.delivered
            .lock()
            .unwrap()
            .push((email.into(), code.into()));
        Ok(())
    }
}
pub struct Fixture {
    pub _root: tempfile::TempDir,
    pub admin: Router,
    pub portal: Router,
    pub store: Arc<AuthStore>,
    pub mailer: Arc<Mailer>,
    pub admin_state: ApiState,
    pub assistants: Arc<Assistants>,
}
pub async fn fixture() -> Fixture {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let suffix = format!(
        "{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let suffix_copy = suffix.clone();
    let (store, execution) = tokio::task::spawn_blocking(move || {
        let url = std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL")
            .expect("isolated PostgreSQL required");
        let db = Database::connect(&url, None, &format!("pro_http_auth_{suffix_copy}")).unwrap();
        let store = Arc::new(AuthStore::open(db).unwrap());
        let execution =
            PostgresStore::connect(&url, &format!("pro_http_auth_runs_{suffix_copy}")).unwrap();
        execution.migrate().unwrap();
        (store, execution)
    })
    .await
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let profiles=[("xiaolan",true,true),("coder",true,true),("inactive",false,true),("private",true,false)].into_iter().map(|(id,enabled,published)|json!({"id":id,"enabled":enabled,"active_revision":1,"created_at":1,"updated_at":1,"revisions":[{"id":id,"revision":1,"display_name":id,"avatar_text":"T","description":"test","system_prompt":"Private system instructions","skill_ids":[],"access":{"skills":{"mode":"none"},"mcp":{"mode":"none"},"tools":{"mode":"none"},"models":{"mode":"all"},"published":published},"digest":"sha256:fixture","created_at":1}]})).collect::<Vec<_>>();
    let file = root.join("agent-profiles.json");
    std::fs::write(
        &file,
        json!({"default_profile":"xiaolan","profiles":profiles}).to_string(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let assistants = Arc::new(Assistants::open(root).unwrap());
    let json =
        json!([{"id":"admin","role":"admin","token":ADMIN,"kind":"local_password"}]).to_string();
    let configured_store = store.clone();
    let credentials = Arc::new(
        tokio::task::spawn_blocking(move || {
            Credentials::with_identity(&json, configured_store, true).unwrap()
        })
        .await
        .unwrap(),
    );
    let mailer = Arc::new(Mailer {
        delivered: Mutex::new(Vec::new()),
        fail: AtomicBool::new(false),
    });
    let (_, stop) = watch::channel(false);
    let admin_state = ApiState {
        execution: Arc::new(ExecutionService(execution)),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    let portal_state = ApiState {
        surface: Surface::Portal,
        port: 9798,
        ..admin_state.clone()
    };
    let admin = router(admin_state.clone())
        .merge(identity_router(
            admin_state.clone(),
            store.clone(),
            mailer.clone(),
            assistants.clone(),
        ))
        .merge(configuration_router(
            admin_state.clone(),
            Arc::new(Models::empty()),
            assistants.clone(),
        ));
    let portal = router(portal_state.clone())
        .merge(identity_router(
            portal_state.clone(),
            store.clone(),
            mailer.clone(),
            assistants.clone(),
        ))
        .merge(configuration_router(
            portal_state,
            Arc::new(Models::empty()),
            assistants.clone(),
        ));
    Fixture {
        _root: directory,
        admin,
        portal,
        store,
        mailer,
        admin_state,
        assistants,
    }
}
pub struct Reply {
    pub status: StatusCode,
    pub body: Value,
    pub cookie: Option<String>,
}
pub async fn request(
    router: &Router,
    port: u16,
    method: &str,
    path: &str,
    body: Value,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> Reply {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("host", format!("127.0.0.1:{port}"))
        .header("origin", format!("http://127.0.0.1:{port}"))
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-exoharness-csrf", csrf);
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|h| h.to_str().unwrap().split(';').next().unwrap().to_owned());
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    }
}
pub async fn admin_login(f: &Fixture) -> (String, String) {
    let login = request(
        &f.admin,
        9797,
        "POST",
        "/v1/auth/login",
        json!({"password":ADMIN}),
        None,
        None,
    )
    .await;
    assert_eq!(login.status, StatusCode::OK);
    let cookie = login.cookie.unwrap();
    let session = request(
        &f.admin,
        9797,
        "GET",
        "/v1/auth/session",
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(session.status, StatusCode::OK);
    (
        cookie,
        session.body["csrf_token"].as_str().unwrap().to_owned(),
    )
}
pub async fn register(f: &Fixture) -> (String, String, String) {
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
    assert_eq!(reply.status, StatusCode::ACCEPTED);
    assert!(reply.body.get("code").is_none());
    let code = f.mailer.delivered.lock().unwrap()[0].1.clone();
    let reply=request(&f.portal,9798,"POST","/v1/auth/register",json!({"email":"student@example.com","code":code,"username":"student","display_name":"同学","password":"student-password"}),None,None).await;
    assert_eq!(reply.status, StatusCode::CREATED);
    let id = reply.body["account"]["id"].as_str().unwrap().to_owned();
    let cookie = reply.cookie.unwrap();
    let reply = request(
        &f.portal,
        9798,
        "GET",
        "/v1/auth/session",
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    (
        id,
        cookie,
        reply.body["csrf_token"].as_str().unwrap().to_owned(),
    )
}
