// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// core.rs:

use bytes::Bytes;
use std::collections::HashMap;
use std::fmt;

pub type Hash = [u8; 32];

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

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => write!(f, "none"),
            Value::Int64(n) => write!(f, "{}", n),
            Value::Uint64(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(hash) => write!(f, "\"{}\"", hex::encode(hash)),
            Value::SetWord(hash) => write!(f, "{}:", hex::encode(hash)),
            Value::GetWord(hash) => write!(f, "{}", hex::encode(hash)),
            Value::LitWord(hash) => write!(f, "'{}", hex::encode(hash)),
            Value::Block(values) => {
                write!(f, "[")?;
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{:?}", v)?;
                }
                write!(f, "]")
            }
            Value::Set(values) => {
                write!(f, "#{{")?;
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{:?}", v)?;
                }
                write!(f, "}}")
            }
            Value::Context(pairs) => {
                write!(f, "context [")?;
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}: {:?}", hex::encode(k), v)?;
                }
                write!(f, "]")
            }
        }
    }
}
impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Transaction {{")?;
        writeln!(f, "  blobs: {{")?;
        for (hash, bytes) in &self.blobs.blobs {
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

                // If it looks like UTF-8 string, show preview
                if let Ok(s) = std::str::from_utf8(bytes) {
                    writeln!(f, "      # \"{}\"", s)?;
                }
            }
        }
        writeln!(f, "  }}")?;
        writeln!(f, "  root: {:?}", self.root)?;
        write!(f, "}}")
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

        let block = Block {
            root: Value::Block(values.into_boxed_slice()),
            blobs,
        };

        println!("{:?}", block);
        Ok(())
    }
}
