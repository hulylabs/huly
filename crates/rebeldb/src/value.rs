// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// eval.rs:

use anyhow::{Context, Result};
use bytes::{BufMut, Bytes, BytesMut};
use std::cell::RefCell;
use std::collections::HashMap;
use xxhash_rust::xxh3::xxh3_64;

pub type Hash = [u8; 32];
pub type Bid = u64;

#[derive(Debug)]
pub enum Value<'a, T: Env>
where
    T: Env,
{
    None,

    Uint(u32, &'a T),
    Int(i32),
    Float(f32, &'a T),
    Uint64(u64),
    Int64(i64),
    Float64(f64),

    String(Bid, &'a T), // utf8-encoded string
    Bytes(Bid, &'a T),  // arbitrary bytes
    Hash(Bid, &'a T),   // 256-bit hash
    PubKey(Bid, &'a T), // 256-bit public key

    SetWord(Bid, &'a T),
    GetWord(Bid, &'a T),
    LitWord(Bid, &'a T),

    Block(Box<[Value<'a, T>]>),
    // Context(Box<[(Cav<'a>, Value<'a>)]>),
}

pub trait Env {
    fn hash(&mut self, data: &[u8]) -> Bid;
}

pub struct Builder;

impl Builder {
    pub fn uint<'a, T: Env>(env: &'a mut T, v: u32) -> Value<'a, T> {
        Value::Uint(v, env)
    }

    pub fn float<'a, T: Env>(env: &'a mut T, v: f32) -> Value<'a, T> {
        Value::Float(v, env)
    }

    pub fn string<'a, T: Env>(env: &'a mut T, v: &str) -> Value<'a, T> {
        let hash = env.hash(v.as_bytes());
        Value::String(hash, env)
    }

    // pub fn block(&'a mut self, v: Vec<Value<'a, T>>) -> Value<'a, T> {
    //     Value::Block(v.into_boxed_slice())
    // }
}

#[derive(Debug)]
pub struct SimpleEnv {
    blobs: HashMap<Bid, Vec<u8>>,
}

impl SimpleEnv {
    const FAST_PATH_THRESHOLD: usize = 1024;

    pub fn new() -> Self {
        Self {
            blobs: HashMap::new(),
        }
    }
}

impl Env for SimpleEnv {
    fn hash(&mut self, data: &[u8]) -> Bid {
        if data.len() < Self::FAST_PATH_THRESHOLD {
            // fast path
            let hash = xxh3_64(data);
            self.blobs.insert(hash, data.to_vec());
            hash
        } else {
            0
        }
    }
}

// impl Blobs for MemoryBlobs {
//     fn get(&self, key: &Hash) -> Option<Bytes> {
//         self.blobs
//             .get(key)
//             .map(|v| Bytes::copy_from_slice(v.as_slice()))
//     }

//     fn put(&mut self, data: &[u8]) -> Hash {
//         let hash = *blake3::hash(data).as_bytes();
//         self.blobs.insert(hash, data.to_vec());
//         hash
//     }
// }

const NONE_TAG: u8 = 0;
const UINT_TAG: u8 = 1;
const INT_TAG: u8 = 2;
const FLOAT_TAG: u8 = 3;
const STRING_TAG: u8 = 4;

pub struct BlockBuilder<'a, T>
where
    T: Env,
{
    bytes: BytesMut,
    offsets: Vec<u32>,
    env: &'a mut T,
}

impl<'a, T> BlockBuilder<'a, T>
where
    T: Env,
{
    pub fn new(env: &'a mut T) -> Self {
        Self {
            bytes: BytesMut::new(),
            offsets: Vec::new(),
            env,
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

    pub fn string(&mut self, v: &str) {
        self.bytes.put_u8(STRING_TAG);
        let hash = self.env.hash(v.as_bytes());
        self.bytes.put_u64_le(hash);
        self.offsets.push(self.bytes.len() as u32);
    }
}
//     pub fn build(&mut self) -> Block {
//         for offset in self.offsets.iter().rev() {
//             self.bytes.put_u32_le(*offset);
//         }
//         self.bytes.put_u32_le(self.offsets.len() as u32);
//         Block {
//             bytes: self.bytes.clone().freeze(),
//         }
//     }
// }

// pub struct Block {
//     bytes: Bytes,
// }

// impl Block {
//     pub fn new(bytes: Bytes) -> Self {
//         Self { bytes }
//     }

//     fn read_u32(&self, offset: usize) -> usize {
//         let b0 = self.bytes[offset] as usize;
//         let b1 = self.bytes[offset + 1] as usize;
//         let b2 = self.bytes[offset + 2] as usize;
//         let b3 = self.bytes[offset + 3] as usize;

//         b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
//     }

//     pub fn len(&self) -> Result<usize> {
//         let size = self.bytes.len();
//         if size < std::mem::size_of::<u32>() {
//             Err(anyhow::anyhow!("block is too short (incorrect size)"))
//         } else {
//             Ok(self.read_u32(size - std::mem::size_of::<u32>()))
//         }
//     }

//     pub fn get(&self, index: usize) -> Result<Value> {
//         let item = self.get_item(index)?;
//         let tag = *item.get(0).context("can't read value tag")?;
//         match tag {
//             NONE_TAG => Ok(Value::None),
//             UINT_TAG => {
//                 let value = item.get(1..).context("can't read value")?;
//                 Ok(Value::Uint(u32::from_le_bytes(value.try_into()?)))
//             }
//             INT_TAG => {
//                 let value = item.get(1..).context("can't read value")?;
//                 Ok(Value::Int(i32::from_le_bytes(value.try_into()?)))
//             }
//             FLOAT_TAG => {
//                 let value = item.get(1..).context("can't read value")?;
//                 Ok(Value::Float(f32::from_le_bytes(value.try_into()?)))
//             }
//             STRING_TAG => {
//                 let tag = *item.get(1).context("can't read string length")?;
//                 if tag == HASH_TAG {
//                     let hash = item.get(2..34).context("can't read hash")?;
//                     let hash = Hash::try_from(hash)?;
//                     Ok(Value::String(Cav::Hash(hash)))
//                 } else {
//                     let len = tag as usize;
//                     Ok(Value::String(Cav::Inline(item.slice(2..2 + len))))
//                 }
//             }
//             _ => Err(anyhow::anyhow!("unknown tag")),
//         }
//     }

//     fn get_item(&self, index: usize) -> Result<Bytes> {
//         if index <= self.len()? {
//             let end = self
//                 .bytes
//                 .len()
//                 .checked_sub(std::mem::size_of::<u32>() * (index + 2))
//                 .context("bad offset")?;

//             let end_offset = self.read_u32(end) as usize;
//             let start_offset = if index == 0 {
//                 0
//             } else {
//                 self.read_u32(end + std::mem::size_of::<u32>()) as usize
//             };

//             Ok(self.bytes.slice(start_offset..end_offset))
//         } else {
//             Err(anyhow::anyhow!("index out of bounds"))
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_builder_1() {
        let mut env = RefCell::new(SimpleEnv::new());
        let values = vec![
            Builder::uint(&mut env, 42),
            Builder::float(&mut env, 3.14),
            Builder::string(&mut env, "hello world"),
        ];
        println!("{:?}", values);
    }

    // #[test]
    // fn test_block_builder() -> Result<()> {
    //     let mut blobs = MemoryBlobs::new();
    //     let mut builder = BlockBuilder::new();
    //     builder.uint(42);
    //     builder.float(3.14);
    //     builder.string(&mut blobs, "hello world");
    //     let block = builder.build();

    //     assert_eq!(block.len()?, 3);

    //     println!("{:?}", block.get(0)?);
    //     println!("{:?}", block.get(1)?);
    //     println!("{:?}", block.get(2)?);

    //     println!("{:?}", block.bytes);
    //     println!("{:?}", block.bytes.len());

    //     Ok(())
    // }
}
