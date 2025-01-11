// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// blob.rs:

use std::collections::HashMap;

pub type Hash = [u8; 32];

pub trait Heap {
    fn put(&mut self, data: &[u8]) -> Hash;
}

pub struct TempHeap {
    data: HashMap<Hash, Vec<u8>>,
}

impl Default for TempHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl TempHeap {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

impl Heap for TempHeap {
    fn put(&mut self, data: &[u8]) -> Hash {
        let hash = *blake3::hash(data).as_bytes();
        self.data.insert(hash, data.to_vec());
        hash
    }
}
