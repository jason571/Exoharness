// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use std::env;
use std::fs;
use std::path::PathBuf;

use lettre::message::{Mailbox, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use thiserror::Error;

#[cfg(test)]
#[path = "../tests/unit/mailer.rs"]
mod tests;

const SMTP_HOST_ENV: &str = "EXOHARNESS_SMTP_HOST";
const SMTP_PORT_ENV: &str = "EXOHARNESS_SMTP_PORT";
const SMTP_USERNAME_ENV: &str = "EXOHARNESS_SMTP_USERNAME";
const SMTP_PASSWORD_FILE_ENV: &str = "EXOHARNESS_SMTP_PASSWORD_FILE";
const SMTP_FROM_ENV: &str = "EXOHARNESS_SMTP_FROM";
const SMTP_TLS_ENV: &str = "EXOHARNESS_SMTP_TLS";

#[derive(Debug, Error)]
pub enum MailerError {
    #[error("SMTP configuration is invalid: {0}")]
    Configuration(String),
    #[error("registration email could not be built")]
    Message,
    #[error("registration email could not be sent")]
    Send,
}

pub struct RegistrationEmailMessage {
    subject: String,
    body: String,
}

impl RegistrationEmailMessage {
    pub fn new(subject: String, body: String) -> Self {
        Self { subject, body }
    }
}

pub trait RegistrationMailer: Send + Sync {
    fn is_configured(&self) -> bool;
    fn send_registration_code(
        &self,
        email: &str,
        code: &str,
        message: &RegistrationEmailMessage,
    ) -> Result<(), MailerError>;
    fn send_password_reset(
        &self,
        _email: &str,
        _link: &str,
        _message: &RegistrationEmailMessage,
    ) -> Result<(), MailerError> {
        Err(MailerError::Message)
    }
}

pub struct DisabledMailer;
pub struct MailService(pub std::sync::Arc<dyn RegistrationMailer>);
impl exoharness_core::services::Service for MailService {
    const KEY: &'static str = "registration-mailer";
}

impl RegistrationMailer for DisabledMailer {
    fn is_configured(&self) -> bool {
        false
    }

    fn send_registration_code(
        &self,
        _email: &str,
        _code: &str,
        _message: &RegistrationEmailMessage,
    ) -> Result<(), MailerError> {
        Err(MailerError::Configuration("SMTP is not configured".into()))
    }
}

pub struct SmtpMailer {
    transport: SmtpTransport,
    from: Mailbox,
}

impl SmtpMailer {
    pub fn from_env() -> Result<Option<Self>, MailerError> {
        let host = optional_env(SMTP_HOST_ENV);
        let from = optional_env(SMTP_FROM_ENV);
        if host.is_none() && from.is_none() {
            return Ok(None);
        }
        let host =
            host.ok_or_else(|| MailerError::Configuration(format!("{SMTP_HOST_ENV} is required")))?;
        let from =
            from.ok_or_else(|| MailerError::Configuration(format!("{SMTP_FROM_ENV} is required")))?;
        let from = from.parse::<Mailbox>().map_err(|_| {
            MailerError::Configuration(format!("{SMTP_FROM_ENV} is not a valid mailbox"))
        })?;
        let tls = optional_env(SMTP_TLS_ENV).unwrap_or_else(|| "starttls".into());
        let mut builder = match tls.to_ascii_lowercase().as_str() {
            "starttls" | "required" => SmtpTransport::starttls_relay(&host),
            "implicit" | "tls" => SmtpTransport::relay(&host),
            _ => {
                return Err(MailerError::Configuration(format!(
                    "{SMTP_TLS_ENV} must be starttls or implicit"
                )));
            }
        }
        .map_err(|_| MailerError::Configuration("SMTP host is invalid".into()))?;
        if let Some(port) = optional_env(SMTP_PORT_ENV) {
            let port = port.parse::<u16>().map_err(|_| {
                MailerError::Configuration(format!("{SMTP_PORT_ENV} must be a valid port"))
            })?;
            builder = builder.port(port);
        }
        match (
            optional_env(SMTP_USERNAME_ENV),
            env::var_os(SMTP_PASSWORD_FILE_ENV).map(PathBuf::from),
        ) {
            (None, None) => {}
            (Some(username), Some(password_file)) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = fs::metadata(&password_file)
                        .map_err(|_| {
                            MailerError::Configuration("SMTP secret file cannot be read".into())
                        })?
                        .permissions()
                        .mode();
                    if mode & 0o077 != 0 {
                        return Err(MailerError::Configuration(
                            "SMTP secret file must have mode 0600".into(),
                        ));
                    }
                }
                let password = fs::read_to_string(&password_file).map_err(|error| {
                    MailerError::Configuration(format!(
                        "could not read {SMTP_PASSWORD_FILE_ENV} from {}: {error}",
                        password_file.display()
                    ))
                })?;
                let password = password.trim_end_matches(['\r', '\n']);
                if password.is_empty() {
                    return Err(MailerError::Configuration(format!(
                        "{} must not be empty",
                        password_file.display()
                    )));
                }
                builder = builder.credentials(Credentials::new(username, password.to_owned()));
            }
            _ => {
                return Err(MailerError::Configuration(format!(
                    "{SMTP_USERNAME_ENV} and {SMTP_PASSWORD_FILE_ENV} must be configured together"
                )));
            }
        }
        Ok(Some(Self {
            transport: builder
                .timeout(Some(std::time::Duration::from_secs(30)))
                .build(),
            from,
        }))
    }
}

impl RegistrationMailer for SmtpMailer {
    fn is_configured(&self) -> bool {
        true
    }

    fn send_registration_code(
        &self,
        email: &str,
        _code: &str,
        registration: &RegistrationEmailMessage,
    ) -> Result<(), MailerError> {
        let recipient = email.parse::<Mailbox>().map_err(|_| MailerError::Message)?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject(&registration.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(registration.body.clone())
            .map_err(|_| MailerError::Message)?;
        self.transport
            .send(&message)
            .map(|_| ())
            .map_err(|_| MailerError::Send)
    }
    fn send_password_reset(
        &self,
        email: &str,
        _link: &str,
        message: &RegistrationEmailMessage,
    ) -> Result<(), MailerError> {
        self.send_registration_code(email, "", message)
    }
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
