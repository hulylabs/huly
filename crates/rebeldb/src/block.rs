// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// block.rs:

use crate::blob::Blobs;
use crate::core::{Content, Hash, InlineBytes, Value};
use anyhow::{Context, Result};
use bytes::{BufMut, Bytes, BytesMut};

const NONE_TAG: u8 = 0;
const UINT_TAG: u8 = 1;
const INT_TAG: u8 = 2;
const FLOAT_TAG: u8 = 3;
const STRING_TAG: u8 = 4;

const HASH_TAG: u8 = 0x80;

pub struct BlockBuilder {
    bytes: BytesMut,
    offsets: Vec<u32>,
}

impl BlockBuilder {
    pub fn new() -> Self {
        Self {
            bytes: BytesMut::new(),
            offsets: Vec::new(),
        }
    }

    pub fn uint(&mut self, v: u32) {
        self.bytes.put_u8(UINT_TAG);
        self.bytes.put_u32_le(v);
        self.offsets.push(self.bytes.len() as u32);
    }

    pub fn float(&mut self, v: f32) {
        self.bytes.put_u8(FLOAT_TAG);
        self.bytes.put_f32_le(v);
        self.offsets.push(self.bytes.len() as u32);
    }

    pub fn string(&mut self, blobs: &mut impl Blobs, v: &str) {
        self.bytes.put_u8(STRING_TAG);
        if v.len() <= std::mem::size_of::<InlineBytes>() {
            self.bytes.put_u8(v.len() as u8);
            self.bytes.put(v.as_bytes());
        } else {
            let hash = blobs.put(v.as_bytes());
            self.bytes.put_u8(HASH_TAG);
            self.bytes.put(hash.as_slice());
        }
        self.offsets.push(self.bytes.len() as u32);
    }

    pub fn build(&mut self) -> Block {
        for offset in self.offsets.iter().rev() {
            self.bytes.put_u32_le(*offset);
        }
        self.bytes.put_u32_le(self.offsets.len() as u32);
        Block {
            bytes: self.bytes.clone().freeze(),
        }
    }
}

pub struct Block {
    bytes: Bytes,
}

impl Block {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    fn read_u32(&self, offset: usize) -> usize {
        let b0 = self.bytes[offset] as usize;
        let b1 = self.bytes[offset + 1] as usize;
        let b2 = self.bytes[offset + 2] as usize;
        let b3 = self.bytes[offset + 3] as usize;

        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    pub fn len(&self) -> Result<usize> {
        let size = self.bytes.len();
        if size < std::mem::size_of::<u32>() {
            Err(anyhow::anyhow!("block is too short (incorrect size)"))
        } else {
            Ok(self.read_u32(size - std::mem::size_of::<u32>()))
        }
    }

    pub fn get(&self, index: usize) -> Result<Value> {
        let item = self.get_item(index)?;
        let tag = *item.get(0).context("can't read value tag")?;
        match tag {
            NONE_TAG => Ok(Value::None),
            UINT_TAG => {
                let mut buf: [u8; 4] = [0; 4];
                buf.copy_from_slice(&item[1..5]);
                Ok(Value::Uint(u32::from_le_bytes(buf)))
            }
            INT_TAG => {
                let mut buf: [u8; 4] = [0; 4];
                buf.copy_from_slice(&item[1..5]);
                Ok(Value::Int(i32::from_le_bytes(buf)))
            }
            FLOAT_TAG => {
                let mut buf: [u8; 4] = [0; 4];
                buf.copy_from_slice(&item[1..5]);
                Ok(Value::Float(f32::from_le_bytes(buf)))
            }
            STRING_TAG => {
                let mut tag: [u8; 1] = [0; 1];
                tag.copy_from_slice(&item[1..2]);
                if tag[0] == HASH_TAG {
                    let mut hash: Hash = [0; 32];
                    hash.copy_from_slice(&item[2..34]);
                    Ok(Value::String(Content::Hash(hash)))
                } else {
                    let len = tag[0] as usize;
                    let mut buf: InlineBytes = [0; 37];
                    buf[..len].copy_from_slice(&item[2..2 + len]);
                    Ok(Value::String(Content::Inline((tag[0], buf))))
                }
            }
            _ => Err(anyhow::anyhow!("unknown tag")),
        }
    }

    fn get_item(&self, index: usize) -> Result<Bytes> {
        if index < self.len()? {
            let end = self
                .bytes
                .len()
                .checked_sub(std::mem::size_of::<u32>() * (index + 2))
                .context("bad offset")?;

            let end_offset = self.read_u32(end) as usize;
            let start_offset = if index == 0 {
                0
            } else {
                self.read_u32(end + std::mem::size_of::<u32>()) as usize
            };

            Ok(self.bytes.slice(start_offset..end_offset))
        } else {
            Err(anyhow::anyhow!("index out of bounds {}", index))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    struct NullBlobs;

    impl Blobs for NullBlobs {
        fn get(&self, _key: &Hash) -> Option<Bytes> {
            unreachable!()
        }

        fn put(&mut self, _data: &[u8]) -> Hash {
            unreachable!()
        }
    }

    #[test]
    fn test_block_builder() -> Result<()> {
        let mut blobs = NullBlobs {};
        let mut builder = BlockBuilder::new();
        builder.uint(99);
        builder.float(3.14);
        builder.string(&mut blobs, "hello world");
        builder.uint(55);
        let block = builder.build();

        // assert_eq!(block.len()?, 3);

        println!("{:?}", block.get(0)?);
        println!("{:?}", block.get(1)?);
        println!("{:?}", block.get(2)?);
        println!("{:?}", block.get(3)?);

        Ok(())
    }
}
