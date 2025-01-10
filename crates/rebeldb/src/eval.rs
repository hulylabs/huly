// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// eval.rs:

use anyhow::{Context, Result};
use bytes::{BufMut, Bytes, BytesMut};
use std::collections::HashMap;

pub type Hash = [u8; 32];

#[derive(Debug)]
pub enum Cav {
    Inline(Bytes),
    Hash(Hash),
}

#[derive(Debug)]
pub enum Value {
    None,

    Uint(u32),
    Int(i32),
    Float(f32),
    Uint64(u64),
    Int64(i64),
    Float64(f64),

    String(Cav),
    SetWord(Cav),
    GetWord(Cav),
    LitWord(Cav),

    Block(Box<[Value]>),
    // Context(Box<[(Cav<'a>, Value<'a>)]>),
}

pub trait Blobs {
    fn get(&self, key: &Hash) -> Option<Bytes>;
    fn put(&mut self, data: &[u8]) -> Hash;
}

pub struct MemoryBlobs {
    blobs: HashMap<Hash, Vec<u8>>,
}

impl MemoryBlobs {
    pub fn new() -> Self {
        Self {
            blobs: HashMap::new(),
        }
    }
}

impl Blobs for MemoryBlobs {
    fn get(&self, key: &Hash) -> Option<Bytes> {
        self.blobs
            .get(key)
            .map(|v| Bytes::copy_from_slice(v.as_slice()))
    }

    fn put(&mut self, data: &[u8]) -> Hash {
        let hash = *blake3::hash(data).as_bytes();
        self.blobs.insert(hash, data.to_vec());
        hash
    }
}

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
    const INLINE_THRESHOLD: usize = 38;

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
        if v.len() <= Self::INLINE_THRESHOLD {
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
                let value = item.get(1..).context("can't read value")?;
                Ok(Value::Uint(u32::from_le_bytes(value.try_into()?)))
            }
            INT_TAG => {
                let value = item.get(1..).context("can't read value")?;
                Ok(Value::Int(i32::from_le_bytes(value.try_into()?)))
            }
            FLOAT_TAG => {
                let value = item.get(1..).context("can't read value")?;
                Ok(Value::Float(f32::from_le_bytes(value.try_into()?)))
            }
            STRING_TAG => {
                let tag = *item.get(1).context("can't read string length")?;
                if tag == HASH_TAG {
                    let hash = item.get(2..34).context("can't read hash")?;
                    let hash = Hash::try_from(hash)?;
                    Ok(Value::String(Cav::Hash(hash)))
                } else {
                    let len = tag as usize;
                    Ok(Value::String(Cav::Inline(item.slice(2..2 + len))))
                }
            }
            _ => Err(anyhow::anyhow!("unknown tag")),
        }
    }

    fn get_item(&self, index: usize) -> Result<Bytes> {
        if index <= self.len()? {
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
            Err(anyhow::anyhow!("index out of bounds"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_block_builder() -> Result<()> {
        let mut blobs = MemoryBlobs::new();
        let mut builder = BlockBuilder::new();
        builder.uint(42);
        builder.float(3.14);
        builder.string(&mut blobs, "hello world");
        let block = builder.build();

        assert_eq!(block.len()?, 3);

        println!("{:?}", block.get(0)?);
        println!("{:?}", block.get(1)?);
        println!("{:?}", block.get(2)?);

        println!("{:?}", block.bytes);
        println!("{:?}", block.bytes.len());

        Ok(())
    }
}
