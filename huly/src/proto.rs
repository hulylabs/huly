//

use crate::{AccId, BlobId, NodeId, OrgId, PublicKey, Uuid};
use anyhow::Result;
use bytes::Bytes;
use chrono::{DateTime, TimeZone, Utc};
use ed25519_dalek::Signature;
use iroh::SecretKey;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

//

//

#[derive(Debug, Serialize, Deserialize)]
pub enum Data {
    Bytes(Bytes),
    Blob(BlobId),
}

impl Data {
    pub fn decode<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        match &self {
            Self::Bytes(bytes) => postcard::from_bytes(bytes.as_ref()).map_err(Into::into),
            Self::Blob(_) => anyhow::bail!("blob decoding not implemented"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedMessage {
    message_type: Uuid,
    format: Uuid,
    data: Data,
    by: PublicKey,
    signature: Signature,
}

impl SignedMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<SignedMessage> {
        let signed_message: Self = postcard::from_bytes(bytes)?;
        signed_message.verify().map(|_| signed_message)
    }

    pub fn verify(&self) -> Result<()> {
        let key = iroh::PublicKey::from_bytes(&self.by)?;
        if let Data::Bytes(data) = &self.data {
            key.verify(&data, &self.signature)?;
            Ok(())
        } else {
            anyhow::bail!("blob verification not implemented");
        }
    }

    pub fn get_signer(&self) -> &PublicKey {
        &self.by
    }

    pub fn get_type(&self) -> Uuid {
        self.message_type
    }

    pub fn decode<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.data.decode()
    }
}

pub struct TypedMessage<T> {
    message: SignedMessage,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TypedMessage<T>
where
    T: DeserializeOwned + Serialize,
{
    pub fn verify_and_decode(&self) -> Result<T> {
        self.message.verify()?;
        self.message.decode::<T>()
    }

    pub fn sign_and_encode(secret_key: &SecretKey, message: T) -> Result<Bytes> {
        let data: Bytes = postcard::to_stdvec(&message)?.into();
        let signature = secret_key.sign(&data);
        let by = secret_key.public();
        let signed_message = SignedMessage {
            from,
            data,
            signature,
        };
        let encoded = postcard::to_stdvec(&signed_message)?;
        Ok(encoded.into())
    }
}

//

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceOwnership {
    account: AccId,
    device: NodeId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MembershipRequest {
    device_ownership: DeviceOwnership,
    org: OrgId,
}

impl MembershipRequest {
    // 001d1f0f-29ba-4812-92e3-0bc02ce1ccc0
    pub const TYPE: Uuid = [
        0x00, 0x1d, 0x1f, 0x0f, 0x29, 0xba, 0x48, 0x12, 0x92, 0xe3, 0x0b, 0xc0, 0x2c, 0xe1, 0xcc,
        0xc0,
    ];

    pub fn new(device: NodeId, account: AccId, org: OrgId) -> Self {
        Self {
            device_ownership: DeviceOwnership { account, device },
            org,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct MembershipResponse {
    request: SignedMessage,
    accepted: bool,
    expiration: Option<Timestamp>,
}

//
