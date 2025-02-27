// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::CoreError;
use crate::module::{BlobStore, Hash};
use std::collections::HashMap;

/// An in-memory implementation of BlobStore for testing and development
pub struct MemoryBlobStore {
    blobs: HashMap<Hash, Vec<u8>>,
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
            store.blobs.insert(hash, data);
        }
        store
    }
}

impl BlobStore for MemoryBlobStore {
    /// Get blob data for a given hash
    fn get(&self, hash: &Hash) -> Result<&[u8], CoreError> {
        self.blobs.get(hash).map(|v| v.as_slice()).ok_or(CoreError::BlobNotFound)
    }
    
    /// Store blob data and return its hash
    fn put(&mut self, data: &[u8]) -> Result<Hash, CoreError> {
        use sha2::{Sha256, Digest};
        
        // Calculate hash
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        
        // Convert to our Hash type
        let mut result = [0u8; 32];
        result.copy_from_slice(&hash);
        
        // Store the data
        self.blobs.insert(result, data.to_vec());
        
        Ok(result)
    }
}