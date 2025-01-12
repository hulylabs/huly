// Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// config.rs

use iroh::{PublicKey, SecretKey};
use once_cell::sync::OnceCell;

static NODE_CONFIG: OnceCell<Config> = OnceCell::new();

#[derive(Debug)]
pub struct Config {
    secret_key: SecretKey,
}

impl Config {
    pub fn new(secret_key: [u8; 32]) -> Self {
        Self {
            secret_key: SecretKey::from(secret_key),
        }
    }

    pub fn public(&self) -> PublicKey {
        self.secret_key.public()
    }
}

pub fn initialize(secret_key: [u8; 32]) {
    NODE_CONFIG.set(Config::new(secret_key)).unwrap();
}

pub fn get_state() -> &'static Config {
    NODE_CONFIG.get().expect("node not configured")
}
