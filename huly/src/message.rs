//

use crate::id::{Hash, PKey};
use anyhow::{Context, Result};
use bytes::{Buf, Bytes, BytesMut};
use chrono::{DateTime, TimeZone, Utc};
use ed25519_dalek::Signature;
use iroh::{PublicKey, SecretKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter};

type Tag = u64;
type Format = u8;

pub const UNDEFINED_FORMAT: Format = 0x0000;
pub const POSTCARD_FORMAT: Format = 0x0001;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Data {
    Blob(Hash),
    Inline(Bytes),
}

impl Data {
    pub fn decode<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        match &self {
            Self::Inline(bytes) => postcard::from_bytes(bytes.as_ref()).map_err(Into::into),
            Self::Blob(_) => Err(anyhow::anyhow!("blob decoding not implemented")),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Inline(bytes) => bytes.as_ref(),
            Self::Blob(_) => panic!("blob decoding not implemented"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    message_type: Tag,
    data_format: Format,
    data: Data,
}

impl Message {
    const MAX_MESSAGE_SIZE: usize = 0x10000;
    const HEADER_SIZE: usize = 10;

    pub async fn read_async(mut reader: impl AsyncRead + Unpin) -> Result<Self> {
        let mut header = [0u8; Self::HEADER_SIZE];
        reader.read_exact(&mut header).await?;

        let mut header = Bytes::copy_from_slice(&header);
        let tag = header.get_u64_le();
        let format = header.get_u8();
        let is_inline = header.get_u8() == 0;

        if is_inline {
            let size = reader.read_u16_le().await?;
            let size = usize::try_from(size).context("frame larger than usize")?;
            if size > Self::MAX_MESSAGE_SIZE {
                anyhow::bail!("Incoming message exceeds the maximum message size");
            }
            let mut buffer = BytesMut::with_capacity(size);
            let mut remaining = size;

            while remaining > 0 {
                let r = reader.read_buf(&mut buffer).await?;
                if r == 0 {
                    anyhow::bail!("Unexpected EOF");
                }
                remaining = remaining.saturating_sub(r);
            }
            Ok(Message {
                message_type: tag,
                data_format: format,
                data: Data::Inline(buffer.freeze()),
            })
        } else {
            let mut hash: [u8; 32] = [0; 32];
            let _ = reader.read_exact(&mut hash).await?;
            Ok(Message {
                message_type: tag,
                data_format: format,
                data: Data::Blob(hash),
            })
        }
    }

    pub async fn write_async(&self, writer: impl AsyncWrite + Unpin) -> Result<()> {
        let mut writer = BufWriter::new(writer);

        writer.write_u64_le(self.message_type).await?;
        writer.write_u8(self.data_format).await?;
        match &self.data {
            Data::Inline(bytes) => {
                writer.write_u8(0).await?;
                writer.write_u16_le(bytes.len() as u16).await?;
                writer.write_all(bytes).await?;
            }
            Data::Blob(hash) => {
                writer.write_u8(0xff).await?;
                writer.write_all(hash).await?;
            }
        }

        writer.flush().await?;
        Ok(())
    }

    pub fn get_type(&self) -> Tag {
        self.message_type
    }

    pub fn get_payload<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.data.decode::<T>()
    }
}

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

    pub fn encode(message: &T) -> Result<Message> {
        Ok(Message {
            message_type: Self::TAG,
            data_format: POSTCARD_FORMAT,
            data: Data::Inline(postcard::to_stdvec(message)?.into()),
        })
    }

    pub fn decode(message: &Message) -> Result<T> {
        if message.get_type() != Self::TAG {
            Err(anyhow::anyhow!("unexpected message type"))
        } else {
            message.data.decode()
        }
    }
}

//

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedMessage {
    message: Message,
    by: PKey,
    signature: Signature,
}

impl SignedMessage {
    pub fn sign(secret_key: &SecretKey, message: Message) -> Result<Self> {
        let signature = secret_key.sign(message.data.as_bytes());
        Ok(SignedMessage {
            message,
            signature,
            by: secret_key.public().into(),
        })
    }

    pub fn verify(&self) -> Result<PKey> {
        let key: PublicKey = self.by.into();
        key.verify(self.message.data.as_bytes(), &self.signature)?;
        Ok(self.by)
    }

    pub fn get_message(&self) -> &Message {
        &self.message
    }
}

pub type SignedMessageType = crate::message::MessageType<SignedMessage, 0x131C5_FACADE_699EA>;

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

// async fn read_lp(
//     mut reader: impl AsyncRead + Unpin,
//     buffer: &mut BytesMut,
//     max_message_size: usize,
// ) -> Result<Option<Bytes>> {
//     let size = match reader.read_u32().await {
//         Ok(size) => size,
//         Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
//         Err(err) => return Err(err.into()),
//     };
//     let mut reader = reader.take(size as u64);
//     let size = usize::try_from(size).context("frame larger than usize")?;
//     if size > max_message_size {
//         anyhow::bail!(
//             "Incoming message exceeds the maximum message size of {max_message_size} bytes"
//         );
//     }
//     buffer.reserve(size);
//     loop {
//         let r = reader.read_buf(buffer).await?;
//         if r == 0 {
//             break;
//         }
//     }
//     Ok(Some(buffer.split_to(size).freeze()))
// }

// async fn write_lp(
//     mut writer: impl AsyncWrite + Unpin,
//     buffer: &Bytes,
//     max_message_size: usize,
// ) -> Result<()> {
//     let size = if buffer.len() > max_message_size {
//         anyhow::bail!("message too large");
//     } else {
//         buffer.len() as u32
//     };
//     writer.write_u32(size).await?;
//     writer.write_all(&buffer).await?;
//     Ok(())
// }
