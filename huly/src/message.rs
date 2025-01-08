//

use crate::id::{Hash, PKey};
use anyhow::{Context, Result};
use bytes::{Bytes, BytesMut};
use chrono::{DateTime, TimeZone, Utc};
use ed25519_dalek::Signature;
use iroh::{PublicKey, SecretKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

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

    fn format(message: &T) -> Result<(Tag, Data)> {
        Ok((Self::TAG, Data::Bytes(postcard::to_stdvec(message)?.into())))
    }

    fn encode_message(format: (Tag, Data), signature: Option<(PKey, Signature)>) -> Result<Bytes> {
        let message = Message {
            message_type: format.0,
            format: POSTCARD_FORMAT,
            data: format.1,
            signature,
        };
        let encoded = postcard::to_stdvec(&message)?;
        Ok(encoded.into())
    }

    pub fn encode(message: &T) -> Result<Bytes> {
        Self::encode_message(Self::format(message)?, None)
    }

    pub fn sign_and_encode(secret_key: &SecretKey, message: &T) -> Result<Bytes> {
        let format = Self::format(message)?;
        let signature = format.1.sign(secret_key)?;
        Self::encode_message(format, Some(signature))
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Data {
    Bytes(Bytes),
    Blob(Hash),
}

impl Data {
    fn sign(&self, secret_key: &SecretKey) -> Result<(PKey, Signature)> {
        match self {
            Self::Bytes(bytes) => Ok((secret_key.public().into(), secret_key.sign(bytes))),
            Self::Blob(_) => Err(anyhow::anyhow!("blob signing not implemented")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    message_type: Tag,
    format: Tag,
    data: Data,
    signature: Option<(PKey, Signature)>,
}

impl Message {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        postcard::from_bytes(bytes).map_err(Into::into)
    }
}

impl SignedMessage {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        postcard::from_bytes(bytes).map_err(Into::into)
    }

    pub fn verify(&self) -> Result<()> {
        let key: PublicKey = self.signature.0.into();
        let data = match &self.message.data {
            Data::Bytes(bytes) => bytes,
            Data::Blob(_) => anyhow::bail!("blob verification not implemented"),
        };
        key.verify(&data, &self.signature.1).map_err(Into::into)
    }

    pub fn get_signer(&self) -> PKey {
        self.signature.0
    }

    pub fn get_type(&self) -> Tag {
        self.message.message_type
    }
}

impl Data {
    pub fn decode<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        match &self {
            Self::Bytes(bytes) => postcard::from_bytes(bytes.as_ref()).map_err(Into::into),
            Self::Blob(_) => Err(anyhow::anyhow!("blob decoding not implemented")),
        }
    }
}

// Timestamp

#[derive(Debug, Serialize, Deserialize)]
pub struct Timestamp(i64);

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

pub async fn read_lp(
    mut reader: impl AsyncRead + Unpin,
    buffer: &mut BytesMut,
    max_message_size: usize,
) -> Result<Option<Bytes>> {
    let size = match reader.read_u32().await {
        Ok(size) => size,
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut reader = reader.take(size as u64);
    let size = usize::try_from(size).context("frame larger than usize")?;
    if size > max_message_size {
        anyhow::bail!(
            "Incoming message exceeds the maximum message size of {max_message_size} bytes"
        );
    }
    buffer.reserve(size);
    loop {
        let r = reader.read_buf(buffer).await?;
        if r == 0 {
            break;
        }
    }
    Ok(Some(buffer.split_to(size).freeze()))
}

pub async fn write_lp(
    mut writer: impl AsyncWrite + Unpin,
    buffer: &Bytes,
    max_message_size: usize,
) -> Result<()> {
    let size = if buffer.len() > max_message_size {
        anyhow::bail!("message too large");
    } else {
        buffer.len() as u32
    };
    writer.write_u32(size).await?;
    writer.write_all(&buffer).await?;
    Ok(())
}
