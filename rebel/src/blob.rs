// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::CoreError;
use crate::module::{BlobStore, Hash};
use std::collections::HashMap;

/// An in-memory implementation of BlobStore for testing and development
pub struct MemoryBlobStore {
    blobs: HashMap<Hash, Box<[u8]>>,
}

impl Default for MemoryBlobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryBlobStore {
    /// Create a new empty MemoryBlobStore
    pub fn new() -> Self {
        Self {
            blobs: HashMap::new(),
        }
    }

    /// Create a new MemoryBlobStore with some initial data
    pub fn with_blobs(initial_data: Vec<(Hash, Vec<u8>)>) -> Self {
        let mut store = Self::new();
        for (hash, data) in initial_data {
            store.blobs.insert(hash, data.into());
        }
        store
    }
}

impl BlobStore for MemoryBlobStore {
    /// Get blob data for a given hash
    fn get(&self, hash: &Hash) -> Result<&[u8], CoreError> {
        self.blobs
            .get(hash)
            .map(|v| v.as_ref())
            .ok_or(CoreError::BlobNotFound)
    }

    /// Store blob data and return its hash
    fn put(&mut self, data: &[u8]) -> Result<Hash, CoreError> {
        let hash = blake3::hash(data);
        let result = *hash.as_bytes();
        self.blobs
            .insert(result, data.to_owned().into_boxed_slice());
        Ok(result)
    }
}
