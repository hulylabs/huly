// Huly™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::fmt;

pub type Hash = [u8; 32];

#[derive(Clone, PartialEq, Eq)]
pub enum Value {
    Uint64(u64),
    Int64(i64),
    String(Hash),
    SetWord(Hash),
    GetWord(Hash),
    LitWord(Hash),
    Block(Box<[Value]>),
    Set(Box<[Value]>),
    Context(Box<[(Hash, Value)]>),
}

pub struct Transaction {
    pub blobs: HashMap<Hash, Vec<u8>>,
    root: Option<Value>,
}

impl Transaction {
    pub fn new() -> Self {
        Self {
            blobs: HashMap::new(),
            root: None,
        }
    }

    fn store_bytes(&mut self, bytes: &[u8]) -> Hash {
        let hash = blake3::hash(bytes).into();
        self.blobs.insert(hash, bytes.to_vec());
        hash
    }

    pub fn string(&mut self, s: &str) -> Value {
        Value::String(self.store_bytes(s.as_bytes()))
    }

    pub fn set_word(&mut self, s: &str) -> Value {
        let hash = self.store_bytes(s.as_bytes());
        Value::SetWord(hash)
    }

    pub fn set_root(&mut self, value: Value) {
        self.root = Some(value);
    }

    // Get all pending blobs that need to be stored
    pub fn pending_blobs(&self) -> &HashMap<Hash, Vec<u8>> {
        &self.blobs
    }

    // Get the root value
    pub fn root(&self) -> Option<&Value> {
        self.root.as_ref()
    }
}

#[macro_export]
macro_rules! block {
    // Empty block
    [] => {
        Value::Block(Box::new([]))
    };

    // Block with values
    [$($x:expr),+ $(,)?] => {{
        let v = vec![$($x),+];
        Value::Block(v.into_boxed_slice())
    }};
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int64(n) => write!(f, "{}", n),
            Value::Uint64(n) => write!(f, "{}", n),
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

impl fmt::Debug for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Transaction {{")?;
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

#[test]
fn test_debug_output() {
    let mut tx = Transaction::new();

    let value = block![
        Value::Int64(42),
        tx.set_word("hello"),
        block![
            Value::Int64(100),
            block![Value::Int64(1), Value::Int64(2), Value::Int64(3)],
            tx.set_word("world")
        ]
    ];

    tx.set_root(value);
    println!("{:#?}", tx);
}

#[test]
fn test_nested_blocks_macro() {
    let mut tx = Transaction::new();

    let value = block![
        Value::Int64(42),
        tx.set_word("x"),
        block![
            Value::Int64(100),
            block![Value::Int64(1), Value::Int64(2), Value::Int64(3)],
            tx.set_word("y")
        ]
    ];

    tx.set_root(value);

    assert_eq!(tx.pending_blobs().len(), 2); // "x" and "y" words

    if let Value::Block(outer) = tx.root().unwrap() {
        assert_eq!(outer.len(), 3);
        assert!(matches!(outer[0], Value::Int64(42)));
        assert!(matches!(outer[1], Value::SetWord(_)));

        if let Value::Block(inner) = &outer[2] {
            assert_eq!(inner.len(), 3);
            assert!(matches!(inner[0], Value::Int64(100)));

            if let Value::Block(most_inner) = &inner[1] {
                assert_eq!(
                    most_inner.as_ref(),
                    &[Value::Int64(1), Value::Int64(2), Value::Int64(3)]
                );
            } else {
                panic!("Expected most inner block");
            }
        } else {
            panic!("Expected inner block");
        }
    } else {
        panic!("Expected outer block");
    }
}
