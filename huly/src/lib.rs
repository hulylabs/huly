// Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

pub type Uuid = [u8; 16];

pub type PublicKey = [u8; 32];
pub type Uid = [u8; 32];
pub type Hash = [u8; 32];

pub type NodeId = PublicKey;
pub type DeviceId = PublicKey;
pub type BlobId = Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjId(Uid);

impl From<Uid> for ObjId {
    fn from(uid: Uid) -> Self {
        Self(uid)
    }
}

impl Borrow<[u8; 32]> for ObjId {
    fn borrow(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<ObjId> for [u8; 32] {
    fn from(id: ObjId) -> Self {
        id.0
    }
}

impl std::fmt::Display for ObjId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex_string: String = self.0.iter().map(|byte| format!("{:02x}", byte)).collect();
        write!(f, "object: {}", hex_string)
    }
}

pub type AccId = ObjId;
pub type OrgId = ObjId;

pub mod db;
pub mod proto;
pub mod server;
