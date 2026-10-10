//! Loopback browser adapter for the unchanged legacy console. Not production auth.
use crate::{Surface, auth::Principal};
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(crate) struct BrowserSession {
    pub principal: Principal,
    pub csrf: String,
    pub account: Option<serde_json::Value>,
    surface: Surface,
    expires: Instant,
}

#[derive(Default)]
pub(crate) struct BrowserSessions(Mutex<BTreeMap<String, BrowserSession>>);

pub(crate) fn cookie_name(surface: Surface) -> &'static str {
    match surface {
        Surface::Management => "exoharness_local_admin",
        Surface::Portal => "exoharness_local_portal",
    }
}

fn nonce() -> Result<String, &'static str> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "session entropy unavailable")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

impl BrowserSessions {
    pub fn issue(
        &self,
        principal: Principal,
        surface: Surface,
    ) -> Result<(String, BrowserSession), &'static str> {
        let mut entries = self.0.lock().map_err(|_| "browser sessions unavailable")?;
        entries.retain(|_, session| session.expires > Instant::now());
        if entries.len() >= 256 {
            return Err("local browser session limit reached");
        }
        let id = nonce()?;
        let session = BrowserSession {
            principal,
            csrf: nonce()?,
            account: None,
            surface,
            expires: Instant::now() + Duration::from_secs(3600),
        };
        entries.insert(id.clone(), session.clone());
        Ok((id, session))
    }
    pub fn get(&self, id: &str, surface: Surface) -> Option<BrowserSession> {
        let mut entries = self.0.lock().ok()?;
        entries.retain(|_, session| session.expires > Instant::now());
        entries
            .get(id)
            .filter(|session| session.surface == surface)
            .cloned()
    }
    pub fn revoke(&self, id: &str, surface: Surface) {
        if let Ok(mut entries) = self.0.lock()
            && entries
                .get(id)
                .is_some_and(|session| session.surface == surface)
        {
            entries.remove(id);
        }
    }
}
impl BrowserSession {
    pub(crate) fn persisted(
        principal: Principal,
        csrf: String,
        surface: Surface,
        account: Option<serde_json::Value>,
    ) -> Self {
        Self {
            principal,
            csrf,
            surface,
            account,
            expires: Instant::now() + Duration::from_secs(1),
        }
    }
}
