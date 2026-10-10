// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    config::Config,
    extensions::{ConfigurationExtension, DatabaseExtension, IdentityExtension, RegistryExtension},
};
use exoharness_api::{ApiState, ExecutionService, Surface, auth::Credentials, router};
use exoharness_configuration::{Assistants, Models, selected_generation};
use exoharness_core::lifecycle::Host;
use exoharness_identity::{
    AuthStore, Database,
    mailer::{DisabledMailer, MailService, RegistrationMailer, SmtpMailer},
};
use exoharness_registry::Registry;
use exoharness_storage_postgres::PostgresStore;
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::watch};

async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {result=tokio::signal::ctrl_c()=>result,_=terminate.recv()=>Ok(())}
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

pub async fn run() -> Result<(), String> {
    let config = Config::load().map_err(str::to_owned)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&config.credentials)
            .map_err(|_| "cannot read local credentials metadata")?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("credentials must have Unix mode 0600".into());
        }
    }
    let credential_data = std::fs::read_to_string(&config.credentials)
        .map_err(|_| "cannot read local credentials")?;
    Credentials::from_json(&credential_data).map_err(str::to_owned)?;
    let (identity, mailer, credentials) = tokio::task::spawn_blocking(move || {
        let identity = Arc::new(
            AuthStore::open(
                Database::from_env("auth").map_err(|_| "identity database connection failed")?,
            )
            .map_err(|_| "identity database setup failed")?,
        );
        let mailer: Arc<dyn RegistrationMailer> =
            match SmtpMailer::from_env().map_err(|_| "SMTP configuration is invalid")? {
                Some(mailer) => Arc::new(mailer),
                None => Arc::new(DisabledMailer),
            };
        let credentials = Arc::new(Credentials::with_identity(
            &credential_data,
            identity.clone(),
            mailer.is_configured(),
        )?);
        Ok::<_, String>((identity, Arc::new(MailService(mailer)), credentials))
    })
    .await
    .map_err(|_| "identity setup worker failed")??;
    // Bind all three ports before starting any server; no half-ready deployment.
    let admin_listener = TcpListener::bind(config.management)
        .await
        .map_err(|_| "management port unavailable")?;
    let portal_listener = TcpListener::bind(config.portal)
        .await
        .map_err(|_| "portal port unavailable")?;
    let media_listener = TcpListener::bind(config.media)
        .await
        .map_err(|_| "media port unavailable")?;
    let namespace = config.namespace;
    let store = tokio::task::spawn_blocking(move || {
        let store = PostgresStore::from_env(&namespace)?;
        store.migrate()?;
        Ok::<_, exoharness_storage_postgres::StoreError>(store)
    })
    .await
    .map_err(|_| "database setup worker failed")?
    .map_err(|e| e.to_string())?;
    let mut host = Host::default();
    host.register(Box::new(IdentityExtension {
        store: identity,
        mailer,
    }))
    .map_err(|e| e.to_string())?;
    let registry = tokio::task::spawn_blocking(|| {
        let store = PostgresStore::from_env("registry")?;
        store.migrate_registry()?;
        Ok::<_, exoharness_storage_postgres::StoreError>(Registry(store))
    })
    .await
    .map_err(|_| "registry worker failed")?
    .map_err(|e| e.to_string())?;
    host.register(Box::new(RegistryExtension(Arc::new(registry))))
        .map_err(|e| e.to_string())?;
    host.register(Box::new(crate::skills_extension::SkillsExtension {
        root: config.capability_directory.clone(),
    }))
    .map_err(|e| e.to_string())?;
    host.register(Box::new(crate::mcp_extension::SecretsExtension {
        root: config.capability_directory.join("secrets"),
    }))
    .map_err(|e| e.to_string())?;
    host.register(Box::new(crate::mcp_extension::McpExtension))
        .map_err(|e| e.to_string())?;
    if let Some(image) = &config.sandbox_image {
        host.register(Box::new(crate::sandbox_extension::SandboxExtension {
            root: config.capability_directory.join("runtime"),
            image: image.clone(),
        }))
        .map_err(|e| e.to_string())?;
        host.register(Box::new(
            crate::skill_runtime_extension::SkillRuntimeExtension {
                root: config.capability_directory.join("skill-runtime"),
                runtime: None,
            },
        ))
        .map_err(|e| e.to_string())?;
    }
    let configuration_base =
        std::env::var_os("EXOHARNESS_CONFIGURATION_DIR").map(std::path::PathBuf::from);
    let model_directory = config.capability_directory.join("models");
    let assistant_directory = config.capability_directory.join("assistants");
    let (models, assistants) = tokio::task::spawn_blocking(move || {
        if let Some(base) = configuration_base
            && let Some(root) = selected_generation(&base)?
        {
            return Ok::<_, exoharness_configuration::ConfigurationError>((
                Models::open(&root)?,
                Assistants::open(&root)?,
            ));
        }
        Ok((
            Models::initialize(&model_directory)?,
            Assistants::initialize(&assistant_directory)?,
        ))
    })
    .await
    .map_err(|_| "configuration worker failed")?
    .map_err(|e| e.to_string())?;
    host.register(Box::new(ConfigurationExtension {
        models: Arc::new(models),
        assistants: Arc::new(assistants),
    }))
    .map_err(|e| e.to_string())?;
    host.register(Box::new(DatabaseExtension(Arc::new(ExecutionService(
        store,
    )))))
    .map_err(|e| e.to_string())?;
    host.register(Box::new(crate::model_extension::ModelExtension::default()))
        .map_err(|e| e.to_string())?;
    host.register(Box::new(crate::chat_extension::ChatExtension::default()))
        .map_err(|e| e.to_string())?;
    host.register(Box::new(crate::tools_extension::ToolsExtension {
        sandbox_enabled: config.sandbox_image.is_some(),
    }))
    .map_err(|e| e.to_string())?;
    host.register(Box::new(crate::artifacts_extension::ArtifactsExtension {
        root: config.capability_directory.join("artifacts"),
        public_base: config.media_public_base.clone(),
    }))
    .map_err(|e| e.to_string())?;
    host.start().await.map_err(|e| e.to_string())?;
    let execution = host
        .service::<ExecutionService>()
        .map_err(|e| e.to_string())?;
    let models = host.service::<Models>().map_err(|e| e.to_string())?;
    let artifacts = host
        .service::<exoharness_artifacts::Artifacts>()
        .map_err(|e| e.to_string())?;
    let model_runtime = host
        .service::<exoharness_model_runtime::ModelRuntime>()
        .map_err(|e| e.to_string())?;
    let chat_tasks = host
        .service::<exoharness_api::ChatTasks>()
        .map_err(|e| e.to_string())?;
    let tool_runtime = host
        .service::<exoharness_tools::ToolRuntime>()
        .map_err(|e| e.to_string())?;
    let assistants = host.service::<Assistants>().map_err(|e| e.to_string())?;
    let registry = host.service::<Registry>().map_err(|e| e.to_string())?;
    let skills = host
        .service::<exoharness_skills::Skills>()
        .map_err(|e| e.to_string())?;
    let identity = host.service::<AuthStore>().map_err(|e| e.to_string())?;
    let sandbox = if config.sandbox_image.is_some() {
        Some(
            host.service::<exoharness_sandbox::Sandbox>()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let mcp = host
        .service::<exoharness_mcp::Mcp>()
        .map_err(|e| e.to_string())?;
    let skill_runtime = if sandbox.is_some() {
        Some(
            host.service::<exoharness_skill_runtime::SkillRuntime>()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let mailer = host.service::<MailService>().map_err(|e| e.to_string())?;
    let (stop_tx, stop_rx) = watch::channel(false);
    let admin_state = ApiState {
        execution: execution.clone(),
        credentials: credentials.clone(),
        surface: Surface::Management,
        port: config.management.port(),
        stop: stop_rx.clone(),
    };
    let portal_state = ApiState {
        execution,
        credentials,
        surface: Surface::Portal,
        port: config.portal.port(),
        stop: stop_rx.clone(),
    };
    let mut admin_stop = stop_rx.clone();
    let mut portal_stop = stop_rx;
    let mut media_stop = admin_stop.clone();
    let admin_artifacts = artifacts.clone();
    let admin_models = models.clone();
    let admin_assistants = assistants.clone();
    let admin_identity = identity.clone();
    let admin_mailer = mailer.clone();
    let portal_model_runtime = model_runtime.clone();
    let portal_sandbox = sandbox.clone();
    let portal_registry = registry.clone();
    let admin_chat_models = models.clone();
    let admin_chat_runtime = model_runtime.clone();
    let admin_chat_tasks = chat_tasks.clone();
    let admin_tools = tool_runtime.clone();
    let mut admin = tokio::spawn(async move {
        let chat = exoharness_api::chat_router_with_tools(
            admin_state.clone(),
            admin_chat_models,
            admin_assistants.clone(),
            admin_chat_runtime,
            admin_chat_tasks,
            Some(admin_tools.clone()),
        );
        let configuration = exoharness_api::configuration_router_with_runtime(
            admin_state.clone(),
            admin_models,
            admin_assistants.clone(),
            Some(registry.clone()),
            Some(model_runtime),
        );
        let identity_routes = exoharness_api::identity_router(
            admin_state.clone(),
            admin_identity,
            admin_mailer.0.clone(),
            admin_assistants.clone(),
        );
        let conversations =
            exoharness_api::conversation_router(admin_state.clone(), admin_assistants.clone());
        let knowledge = exoharness_api::knowledge_router_with_sandbox(
            admin_state.clone(),
            admin_assistants.clone(),
            sandbox.clone(),
        );
        let management = exoharness_api::management_router(admin_state.clone(), admin_assistants);
        let runtime = exoharness_api::runtime_router_with_tools(
            admin_state.clone(),
            sandbox,
            Some(admin_tools),
        );
        let artifact_routes =
            exoharness_api::artifact_management_router(admin_state.clone(), admin_artifacts);
        let skill_routes = exoharness_api::skills_router(admin_state.clone(), skills.clone());
        let mcp_routes = exoharness_api::mcp_router(admin_state.clone(), mcp.clone());
        let registry_routes = exoharness_api::registry_router_with_runtime(
            admin_state.clone(),
            registry,
            skills,
            mcp,
            skill_runtime,
        );
        axum::serve(
            admin_listener,
            router(admin_state)
                .merge(chat)
                .merge(configuration)
                .merge(management)
                .merge(knowledge)
                .merge(conversations)
                .merge(runtime)
                .merge(artifact_routes)
                .merge(identity_routes)
                .merge(registry_routes)
                .merge(mcp_routes)
                .merge(skill_routes),
        )
        .with_graceful_shutdown(async move {
            let _ = admin_stop.changed().await;
        })
        .await
    });
    let mut portal = tokio::spawn(async move {
        let chat = exoharness_api::chat_router_with_tools(
            portal_state.clone(),
            models.clone(),
            assistants.clone(),
            portal_model_runtime.clone(),
            chat_tasks,
            Some(tool_runtime),
        );
        let identity_routes = exoharness_api::identity_router_with_registry(
            portal_state.clone(),
            identity,
            mailer.0.clone(),
            assistants.clone(),
            Some(portal_registry),
        );
        let conversations =
            exoharness_api::conversation_router(portal_state.clone(), assistants.clone());
        let knowledge = exoharness_api::knowledge_router_with_sandbox(
            portal_state.clone(),
            assistants.clone(),
            portal_sandbox,
        );
        let configuration = exoharness_api::configuration_router_with_runtime(
            portal_state.clone(),
            models,
            assistants,
            None,
            Some(portal_model_runtime),
        );
        axum::serve(
            portal_listener,
            router(portal_state)
                .merge(chat)
                .merge(configuration)
                .merge(knowledge)
                .merge(conversations)
                .merge(identity_routes),
        )
        .with_graceful_shutdown(async move {
            let _ = portal_stop.changed().await;
        })
        .await
    });
    let mut media = tokio::spawn(async move {
        axum::serve(
            media_listener,
            exoharness_api::media_router(artifacts, config.media.port()),
        )
        .with_graceful_shutdown(async move {
            let _ = media_stop.changed().await;
        })
        .await
    });
    println!(
        "ExoHarness validation listening on {} / {} / {}; Agent integration incomplete",
        config.management, config.portal, config.media
    );
    let (admin_done, portal_done, media_done, failure) = tokio::select! {
        result=&mut admin=>(true,false,false,Some(format!("management listener ended: {}",result.is_ok_and(|r|r.is_ok())))),
        result=&mut portal=>(false,true,false,Some(format!("portal listener ended: {}",result.is_ok_and(|r|r.is_ok())))),
        result=&mut media=>(false,false,true,Some(format!("media listener ended: {}",result.is_ok_and(|r|r.is_ok())))),
        result=shutdown_signal()=>(false,false,false,result.err().map(|_|"shutdown signal listener failed".into())),
    };
    let _ = stop_tx.send(true);
    let drained = tokio::time::timeout(Duration::from_secs(10), async {
        if !admin_done {
            let _ = (&mut admin).await;
        }
        if !portal_done {
            let _ = (&mut portal).await;
        }
        if !media_done {
            let _ = (&mut media).await;
        }
    })
    .await;
    if drained.is_err() {
        if !admin.is_finished() {
            admin.abort();
            let _ = admin.await;
        }
        if !portal.is_finished() {
            portal.abort();
            let _ = portal.await;
        }
        if !media.is_finished() {
            media.abort();
            let _ = media.await;
        }
    }
    let cleanup = host.shutdown().await;
    if !cleanup.is_empty() {
        return Err("extension cleanup incomplete".into());
    }
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(())
}
