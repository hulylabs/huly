//

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use iroh::PublicKey;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

// we have two types of identities: Hash and PublicKey
// both are represented as 32-byte arrays
pub type Uid = [u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PKey(Uid);

impl PKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<PublicKey> for PKey {
    fn from(key: PublicKey) -> Self {
        Self(*key.as_bytes())
    }
}

impl Into<PublicKey> for PKey {
    fn into(self) -> PublicKey {
        PublicKey::from_bytes(&self.0).expect("no way")
    }
}

impl From<Uid> for PKey {
    fn from(uid: Uid) -> Self {
        Self(uid)
    }
}

impl Borrow<Uid> for PKey {
    fn borrow(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<PKey> for Uid {
    fn from(key: PKey) -> Self {
        key.0
    }
}

impl std::fmt::Display for PKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex_string: String = self
            .as_bytes()
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect();
        write!(f, "public key: {}", hex_string)
    }
}

pub type AccId = PKey;
pub type OrgId = PKey;

pub type NodeId = PKey;
pub type DeviceId = NodeId;

//

#[derive(Debug, Serialize, Deserialize)]
struct Timestamp(i64);

impl From<DateTime<Utc>> for Timestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        Timestamp(dt.timestamp())
    }
}

impl TryInto<DateTime<Utc>> for Timestamp {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<DateTime<Utc>> {
        match Utc.timestamp_opt(self.0, 0) {
            chrono::LocalResult::Single(datetime) => Ok(datetime),
            chrono::LocalResult::None => anyhow::bail!("timestamp is out of range"),
            chrono::LocalResult::Ambiguous(_, _) => anyhow::bail!("timestamp is ambiguous"),
        }
    }
}
