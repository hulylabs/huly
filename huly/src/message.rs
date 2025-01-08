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
    Blob(Hash),
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
