// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Authenticated encryption for administrator-revealable application keys.
//! The private master key is separate from database backups and API responses.
use crate::{
    Result, SecretError,
    filesystem::{directory, private_file},
};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use std::{fs, io::Write, path::Path};

pub struct SecretCipher {
    key: LessSafeKey,
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
impl SecretCipher {
    pub fn open(root: &Path) -> Result<Self> {
        if !root.try_exists()? {
            directory(root.parent().ok_or(SecretError::Invalid)?)?;
        }
        directory(root)?;
        let path = root.join("master-key");
        if !path.try_exists()? {
            let mut key = [0u8; 32];
            getrandom::fill(&mut key).map_err(|_| SecretError::Storage)?;
            let mut file = tempfile::NamedTempFile::new_in(root)?;
            file.write_all(&key)?;
            file.as_file().sync_all()?;
            match file.persist_noclobber(&path) {
                Ok(_) => {
                    #[cfg(unix)]
                    fs::File::open(root)?.sync_all()?;
                }
                Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(SecretError::Storage),
            }
        }
        private_file(&path)?;
        let key = fs::read(path)?;
        if key.len() != 32 {
            return Err(SecretError::Invalid);
        }
        let key = UnboundKey::new(&AES_256_GCM, &key).map_err(|_| SecretError::Invalid)?;
        Ok(Self {
            key: LessSafeKey::new(key),
        })
    }
    pub fn generate_key() -> Result<String> {
        let mut value = [0u8; 32];
        getrandom::fill(&mut value).map_err(|_| SecretError::Storage)?;
        Ok(format!("exo_{}", hex(&value)))
    }
    pub fn seal(&self, context: &str, secret: &str) -> Result<String> {
        if context.is_empty() || context.len() > 256 || secret.is_empty() || secret.len() > 256 {
            return Err(SecretError::Invalid);
        }
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|_| SecretError::Storage)?;
        let mut output = secret.as_bytes().to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(context.as_bytes()),
                &mut output,
            )
            .map_err(|_| SecretError::Storage)?;
        let mut envelope = nonce.to_vec();
        envelope.extend(output);
        Ok(format!("v1.{}", hex(&envelope)))
    }
    pub fn open_secret(&self, context: &str, encrypted: &str) -> Result<String> {
        let raw = encrypted
            .strip_prefix("v1.")
            .filter(|v| v.len() <= 1024 && v.len() % 2 == 0 && v.len() >= 56)
            .ok_or(SecretError::Invalid)?;
        if !raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(SecretError::Invalid);
        }
        let mut bytes = (0..raw.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).map_err(|_| SecretError::Invalid))
            .collect::<Result<Vec<_>>>()?;
        let nonce: [u8; 12] = bytes[..12].try_into().map_err(|_| SecretError::Invalid)?;
        let plain = self
            .key
            .open_in_place(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(context.as_bytes()),
                &mut bytes[12..],
            )
            .map_err(|_| SecretError::Invalid)?;
        String::from_utf8(plain.to_vec()).map_err(|_| SecretError::Invalid)
    }
}
