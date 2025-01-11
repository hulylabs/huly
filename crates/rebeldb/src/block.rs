// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// block.rs:

use crate::blob::Blobs;
use crate::core::{Content, Hash, InlineBytes, Value};
use bytes::{BufMut, BytesMut};

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
            bytes: &self.bytes, //.clone().freeze(),
        }
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> usize {
    let b0 = bytes[offset] as usize;
    let b1 = bytes[offset + 1] as usize;
    let b2 = bytes[offset + 2] as usize;
    let b3 = bytes[offset + 3] as usize;

    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

pub struct Block<'a> {
    bytes: &'a [u8],
}

impl<'a> Block<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub fn len(&self) -> Option<usize> {
        let size = self.bytes.len();
        if size < std::mem::size_of::<u32>() {
            None
        } else {
            Some(read_u32(self.bytes, size - 4))
        }
    }

    pub fn get(&self, index: usize) -> Option<Value> {
        if let Some(item) = self.get_item(index) {
            let tag = item[0];
            match tag {
                NONE_TAG => Some(Value::None),
                UINT_TAG => {
                    let mut buf: [u8; 4] = [0; 4];
                    buf.copy_from_slice(&item[1..5]);
                    Some(Value::Uint(u32::from_le_bytes(buf)))
                }
                INT_TAG => {
                    let mut buf: [u8; 4] = [0; 4];
                    buf.copy_from_slice(&item[1..5]);
                    Some(Value::Int(i32::from_le_bytes(buf)))
                }
                FLOAT_TAG => {
                    let mut buf: [u8; 4] = [0; 4];
                    buf.copy_from_slice(&item[1..5]);
                    Some(Value::Float(f32::from_le_bytes(buf)))
                }
                STRING_TAG => {
                    let mut tag: [u8; 1] = [0; 1];
                    tag.copy_from_slice(&item[1..2]);
                    if tag[0] == HASH_TAG {
                        let mut hash: Hash = [0; 32];
                        hash.copy_from_slice(&item[2..34]);
                        Some(Value::String(Content::Hash(hash)))
                    } else {
                        let len = tag[0] as usize;
                        let mut buf: InlineBytes = [0; 37];
                        buf[..len].copy_from_slice(&item[2..2 + len]);
                        Some(Value::String(Content::Inline((tag[0], buf))))
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn get_item(&self, index: usize) -> Option<&[u8]> {
        let len = self.bytes.len();
        if let Some(count_offset) = len.checked_sub(std::mem::size_of::<u32>()) {
            let count = read_u32(self.bytes, count_offset) as usize;
            if index < count {
                if let Some(end) = count_offset.checked_sub(4 * (index + 1)) {
                    let end_offset = read_u32(self.bytes, end) as usize;
                    let start_offset = if index == 0 {
                        0
                    } else {
                        read_u32(self.bytes, end + 4) as usize
                    };
                    return Some(&self.bytes[start_offset..end_offset]);
                }
            }
        }
        None
    }
}

pub struct BlockIterator<'a> {
    block: &'a [u8],
    count: usize,
    position: usize,
    offset: usize,
}

impl<'a> BlockIterator<'a> {
    pub fn new(block: &'a [u8]) -> Option<Self> {
        let len = block.len();
        if let Some(count_offset) = len.checked_sub(std::mem::size_of::<u32>()) {
            let count = read_u32(block, count_offset) as usize;
            Some(Self {
                block,
                count,
                offset: 0,
                position: 0,
            })
        } else {
            None
        }
    }
}

impl<'a> Iterator for BlockIterator<'a> {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position < self.count {
            if let Some(end) = self.block.len().checked_sub(4 * (self.position + 2)) {
                let end_offset = read_u32(self.block, end) as usize;
                let slice = &self.block[self.offset..end_offset];
                self.offset = end_offset;
                self.position += 1;
                Some(&self.block[start_offset..end_offset])
            } else {
                None
            }
        } else {
            None
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
        builder.uint(199);
        builder.float(3.14);
        builder.string(&mut blobs, "hello world");
        builder.uint(55);
        let block = builder.build();

        // assert_eq!(block.len()?, 3);

        println!("{:?}", block.get(0));
        println!("{:?}", block.get(1));
        println!("{:?}", block.get(2));
        println!("{:?}", block.get(3));

        Ok(())
    }
}
