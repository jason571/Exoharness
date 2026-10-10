use std::{env, net::SocketAddr, path::PathBuf};

pub struct Config {
    pub management: SocketAddr,
    pub portal: SocketAddr,
    pub media: SocketAddr,
    pub media_public_base: String,
    pub credentials: PathBuf,
    pub namespace: String,
    pub capability_directory: PathBuf,
    pub sandbox_image: Option<String>,
}
impl Config {
    pub fn load() -> Result<Self, &'static str> {
        let management = env::var("EXOHARNESS_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9797".into())
            .parse::<SocketAddr>()
            .map_err(|_| "invalid management bind")?;
        let portal = env::var("EXOHARNESS_PORTAL_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9798".into())
            .parse::<SocketAddr>()
            .map_err(|_| "invalid portal bind")?;
        let media = env::var("EXOHARNESS_MEDIA_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9790".into())
            .parse::<SocketAddr>()
            .map_err(|_| "invalid media bind")?;
        if !management.ip().is_loopback()
            || !portal.ip().is_loopback()
            || !management.is_ipv4()
            || !portal.is_ipv4()
            || management.port() == 0
            || portal.port() == 0
            || management.port() == portal.port()
            || !media.ip().is_loopback()
            || !media.is_ipv4()
            || media.port() == 0
            || media.port() == management.port()
            || media.port() == portal.port()
        {
            return Err("local validation requires three distinct IPv4 loopback ports");
        }
        let credentials = env::var_os("EXOHARNESS_LOCAL_CREDENTIALS_FILE")
            .map(PathBuf::from)
            .ok_or("local credentials file is required")?;
        let namespace =
            env::var("EXOHARNESS_DATABASE_NAMESPACE").unwrap_or_else(|_| "execution".into());
        let capability_directory = env::var_os("EXOHARNESS_CAPABILITY_DIRECTORY")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                credentials
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join("capabilities")
            });
        Ok(Self {
            management,
            portal,
            media,
            media_public_base: env::var("EXOHARNESS_MEDIA_PUBLIC_BASE_URL")
                .unwrap_or_else(|_| format!("http://{media}")),
            credentials,
            namespace,
            capability_directory,
            sandbox_image: env::var("EXOHARNESS_SANDBOX_IMAGE")
                .ok()
                .filter(|s| !s.is_empty()),
        })
    }
}
