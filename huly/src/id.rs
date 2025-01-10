// Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use anyhow::Result;
use iroh::PublicKey;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::str::FromStr;

const LENGTH: usize = 32;

// we have two types of identities: Hash and PublicKey
// both are represented as 32-byte arrays
pub type Uid = [u8; 32];
pub type Hash = [u8; 32];

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

impl From<PKey> for PublicKey {
    fn from(val: PKey) -> Self {
        PublicKey::from_bytes(&val.0).expect("no way")
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

impl Display for PKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PublicKey({})", data_encoding::HEXLOWER.encode(&self.0))
    }
}

fn decode_base32_hex(s: &str) -> Result<[u8; 32]> {
    let mut bytes = [0u8; 32];

    let res = if s.len() == LENGTH * 2 {
        data_encoding::HEXLOWER.decode_mut(s.as_bytes(), &mut bytes)
    } else {
        data_encoding::BASE32_NOPAD.decode_mut(s.to_ascii_uppercase().as_bytes(), &mut bytes)
    };
    match res {
        Ok(len) => {
            if len != LENGTH {
                anyhow::bail!("invalid length");
            }
        }
        Err(partial) => return Err(partial.error.into()),
    }
    Ok(bytes)
}

impl FromStr for PKey {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PKey(decode_base32_hex(s)?))
    }
}

pub type AccId = PKey;
pub type OrgId = PKey;

//

pub type ObjId = PKey;
pub type NodeId = PKey;
pub type DeviceId = NodeId;
