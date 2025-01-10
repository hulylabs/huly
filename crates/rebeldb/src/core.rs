// Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// core.rs:

use bytes::Bytes;
use std::collections::HashMap;

pub type Hash = [u8; 32];

#[derive(Debug)]
pub enum Value {
    None,
    Uint64(u64),
    Int64(i64),
    Float(f64),
    String(Hash),
    SetWord(Hash),
    GetWord(Hash),
    LitWord(Hash),
    Block(Box<[Value]>),
    Set(Box<[Value]>),
    Context(Box<[(Hash, Value)]>),
}

impl Value {
    pub fn uint64(v: u64) -> Self {
        Self::Uint64(v)
    }

    pub fn int64(v: i64) -> Self {
        Self::Int64(v)
    }
}

#[derive(Debug, Default)]
pub struct Blobs {
    blobs: HashMap<Hash, Vec<u8>>,
}

impl Blobs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, data: &[u8]) -> Hash {
        let hash = *blake3::hash(data).as_bytes();
        self.blobs.insert(hash, data.to_vec());
        hash
    }

    pub fn get(&self, hash: &Hash) -> Option<&[u8]> {
        self.blobs.get(hash).map(|v| v.as_slice())
    }

    pub fn string(&mut self, bytes: Bytes) -> Value {
        Value::String(self.store(&bytes))
    }

    pub fn set_word(&mut self, bytes: &[u8]) -> Value {
        Value::SetWord(self.store(bytes))
    }

    pub fn get_word(&mut self, bytes: &[u8]) -> Value {
        Value::GetWord(self.store(bytes))
    }

    pub fn lit_word(&mut self, name: &str) -> Value {
        Value::LitWord(self.store(name.as_bytes()))
    }
}

#[derive(Debug)]
pub struct Block {
    root: Value,
    blobs: Blobs,
}

impl Block {
    pub fn new(root: Value, blobs: Blobs) -> Self {
        Self { root, blobs }
    }

    pub fn root(&self) -> &Value {
        &self.root
    }

    pub fn get_blob(&self, hash: &Hash) -> Option<&[u8]> {
        self.blobs.get(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_builder() -> Result<(), String> {
        let mut blobs = Blobs::new();

        // Simply build values directly
        let values = vec![
            blobs.set_word(b"name"),
            // blobs.string("John"),
            blobs.set_word(b"age"),
            Value::int64(30),
        ];

        let x = Block {
            root: Value::Block(values.into_boxed_slice()),
            blobs,
        };

        Ok(())
    }
}
