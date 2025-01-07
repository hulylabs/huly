//

use crate::{AccId, BlobId, DeviceId, OrgId, PublicKey, Uuid};
use anyhow::Result;
use bytes::Bytes;
use chrono::{DateTime, TimeZone, Utc};
use ed25519_dalek::Signature;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

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

//

#[derive(Debug, Serialize, Deserialize)]
pub enum Data {
    Bytes(Bytes),
    Blob(BlobId),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    message_type: Uuid,
    format: Uuid,
    timestamp: Timestamp,
    data: Data,
}

impl Message {
    pub fn get_type(&self) -> Uuid {
        self.message_type
    }

    pub fn decode<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        match &self.data {
            Data::Bytes(bytes) => postcard::from_bytes(bytes.as_ref()).map_err(Into::into),
            Data::Blob(_) => anyhow::bail!("blob decoding not implemented"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedMessage {
    message: Data,
    by: PublicKey,
    signature: Signature,
}

impl SignedMessage {
    pub fn verify_and_decode(bytes: &[u8]) -> Result<(PublicKey, Message)> {
        let signed_message: Self = postcard::from_bytes(bytes)?;
        if let Data::Bytes(data) = &signed_message.message {
            let key = iroh::PublicKey::from_bytes(&signed_message.by)?;
            key.verify(&data, &signed_message.signature)?;
            let message: Message = postcard::from_bytes(&data)?;
            Ok((signed_message.by, message))
        } else {
            anyhow::bail!("blob verification not implemented");
        }
    }

    // pub fn sign_and_encode(secret_key: &SecretKey, message: &Message) -> Result<Bytes> {
    //     let data: Bytes = postcard::to_stdvec(&message)?.into();
    //     let signature = secret_key.sign(&data);
    //     let from: PublicKey = secret_key.public();
    //     let signed_message = Self {
    //         from,
    //         data,
    //         signature,
    //     };
    //     let encoded = postcard::to_stdvec(&signed_message)?;
    //     Ok(encoded.into())
    // }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceOwnership {
    account: AccId,
    device: DeviceId,
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
}

#[derive(Serialize, Deserialize)]
pub struct MembershipResponse {
    request: SignedMessage,
    accepted: bool,
    expiration: Option<Timestamp>,
}

//

pub fn
