//

use crate::{BlobId, PublicKey};
use anyhow::Result;
use bytes::Bytes;
use ed25519_dalek::Signature;
use iroh::SecretKey;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

type Tag = u64;

const POSTCARD_FORMAT: Tag = 0;

pub struct MessageType<T, const ID: Tag>
where
    T: Serialize + DeserializeOwned,
{
    _marker: std::marker::PhantomData<T>,
}

impl<T, const ID: Tag> MessageType<T, ID>
where
    T: Serialize + DeserializeOwned,
{
    pub const TAG: Tag = ID;

    pub fn sign_and_encode(secret_key: &SecretKey, message: T) -> Result<Bytes> {
        let data: Bytes = postcard::to_stdvec(&message)?.into();
        let signature = secret_key.sign(&data);
        let by = secret_key.public().public().to_bytes();
        let signed_message = SignedMessage {
            message_type: Self::TAG,
            format: POSTCARD_FORMAT,
            data: Data::Bytes(data),
            by,
            signature,
        };
        let encoded = postcard::to_stdvec(&signed_message)?;
        Ok(encoded.into())
    }

    pub fn decode(message: SignedMessage) -> Result<T> {
        if message.message_type != Self::TAG {
            anyhow::bail!("unexpected message type");
        }
        message.data.decode()
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Data {
    Bytes(Bytes),
    Blob(BlobId),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedMessage {
    message_type: Tag,
    format: Tag,
    data: Data,
    by: PublicKey,
    signature: Signature,
}

impl SignedMessage {
    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self> {
        let this: Self = postcard::from_bytes(bytes)?;
        let key = iroh::PublicKey::from_bytes(&this.by)?;
        if let Data::Bytes(data) = &this.data {
            key.verify(&data, &this.signature)?;
            Ok(this)
        } else {
            anyhow::bail!("blob verification not implemented");
        }
    }

    pub fn get_signer(&self) -> &PublicKey {
        &self.by
    }

    pub fn get_type(&self) -> Tag {
        self.message_type
    }
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
