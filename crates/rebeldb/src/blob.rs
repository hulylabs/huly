// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// blob.rs:

use crate::core::Hash;
use bytes::Bytes;
use std::collections::HashMap;
use std::fmt;

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

impl fmt::Debug for MemoryBlobs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  blobs: {{")?;
        for (hash, bytes) in &self.blobs {
            writeln!(f, "    {} =>", hex::encode(hash))?;
            // Hexdump-like format, 16 bytes per line
            for chunk in bytes.chunks(16) {
                // Hex part
                write!(f, "      ")?;
                for b in chunk {
                    write!(f, "{:02x} ", b)?;
                }
                // Padding for incomplete last line
                for _ in chunk.len()..16 {
                    write!(f, "   ")?;
                }
                // ASCII part
                write!(f, " |")?;
                for &b in chunk {
                    let c = if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '.'
                    };
                    write!(f, "{}", c)?;
                }
                // Padding for incomplete last line
                for _ in chunk.len()..16 {
                    write!(f, " ")?;
                }
                writeln!(f, "|")?;
            }
        }
        writeln!(f, "  }}")
    }
}
