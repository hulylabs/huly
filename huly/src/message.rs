//

use crate::id::PKey;
use anyhow::Result;
use bytes::Bytes;
use ed25519_dalek::Signature;
use iroh::{PublicKey, SecretKey};
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
        let signed_message = SignedMessage {
            message_type: Self::TAG,
            format: POSTCARD_FORMAT,
            data: Data::Bytes(data),
            by: secret_key.public().into(),
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
    Blob(),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedMessage {
    message_type: Tag,
    format: Tag,
    data: Data,
    by: PKey,
    signature: Signature,
}

impl SignedMessage {
    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self> {
        let this: Self = postcard::from_bytes(bytes)?;
        let key: PublicKey = this.by.into();
        if let Data::Bytes(data) = &this.data {
            key.verify(&data, &this.signature)?;
            Ok(this)
        } else {
            Err(anyhow::anyhow!("blob verification not implemented"))
        }
    }

    pub fn get_signer(&self) -> PKey {
        self.by
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
            Self::Blob() => Err(anyhow::anyhow!("blob decoding not implemented")),
        }
    }
}
