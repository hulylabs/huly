// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// core.rs:

use std::result::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValueError {
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("conversion error")]
    ConversionError,
}

pub type Hash = [u8; 32];
// pub type Symbol = [u8; 32];

const INLINE_CONTENT_LEN: usize = 37;
pub type Inline = (u8, [u8; INLINE_CONTENT_LEN]);
pub type Symbol = Inline;

#[derive(Debug, Clone)]
pub enum Content {
    Inline(Inline),
    Hash(Hash),
}

#[derive(Debug, Clone)]
pub enum Value {
    None,

    Uint(u32),
    Int(i32),
    Float(f32),
    Uint64(u64),
    Int64(i64),
    Float64(f64),

    PubKey(Hash),
    String(Content),

    Word(Symbol),
    SetWord(Symbol),
    GetWord(Symbol),
    LitWord(Symbol),

    Block(Box<[Value]>),
    // Context(Box<[(Cav<'a>, Value<'a>)]>),
    NativeFn(fn(&Vec<Value>) -> Value, usize), // (fn(stack), arity) -> Value
}

pub trait Storage {
    fn put(&mut self, data: &[u8]) -> Hash;
}

// pub trait Context {
//     fn get_storage(&self) -> &impl Storage;
//     // fn get_value(&self, symbol: &Symbol) -> Option<Value>;
//     fn get_stack(&self) -> &Vec<Value>;
// }

impl Value {
    const NONE_TAG: u8 = 0;
    const UINT_TAG: u8 = 1;
    const INT_TAG: u8 = 2;
    const FLOAT_TAG: u8 = 3;
    const STRING_TAG: u8 = 4;

    pub fn from_slice(bytes: &[u8]) -> Option<Value> {
        if bytes.is_empty() {
            return None;
        }
        let tag = bytes[0];
        match tag {
            Self::NONE_TAG => Some(Value::None),
            Self::UINT_TAG => {
                if bytes.len() < 5 {
                    None
                } else {
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&bytes[1..5]);
                    Some(Value::Uint(u32::from_le_bytes(buf)))
                }
            }
            Self::INT_TAG => {
                if bytes.len() < 5 {
                    None
                } else {
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&bytes[1..5]);
                    Some(Value::Int(i32::from_le_bytes(buf)))
                }
            }
            Self::FLOAT_TAG => {
                if bytes.len() < 5 {
                    None
                } else {
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&bytes[1..5]);
                    Some(Value::Float(f32::from_le_bytes(buf)))
                }
            }
            Self::STRING_TAG => {
                if bytes.len() < 2 {
                    None
                } else {
                    let len = bytes[1] as usize;
                    if len > INLINE_CONTENT_LEN {
                        if bytes.len() < 34 {
                            None
                        } else {
                            let mut hash = [0u8; 32];
                            hash.copy_from_slice(&bytes[2..34]);
                            Some(Value::String(Content::Hash(hash)))
                        }
                    } else if bytes.len() < 2 + len {
                        None
                    } else {
                        let mut buf = [0u8; INLINE_CONTENT_LEN];
                        buf[..len].copy_from_slice(&bytes[2..2 + len]);
                        Some(Value::String(Content::Inline((bytes[1], buf))))
                    }
                }
            }
            _ => None,
        }
    }

    pub fn as_str(&self) -> Result<&str, ValueError> {
        match self {
            Value::String(Content::Inline((len, buf))) => {
                std::str::from_utf8(&buf[..*len as usize]).map_err(ValueError::Utf8Error)
            }
            _ => Err(ValueError::ConversionError),
        }
    }

    pub fn as_int(&self) -> Result<i64, ValueError> {
        match self {
            Value::Uint(x) => Ok(*x as i64),
            _ => Err(ValueError::ConversionError),
        }
    }

    pub fn uint(x: u32) -> Self {
        Value::Uint(x)
    }

    pub fn int(x: i32) -> Self {
        Value::Int(x)
    }

    pub fn float(x: f32) -> Self {
        Value::Float(x)
    }

    fn to_symbol(x: &str) -> Symbol {
        assert!(x.len() <= INLINE_CONTENT_LEN);
        let mut buf = [0u8; INLINE_CONTENT_LEN];
        buf[..x.len()].copy_from_slice(x.as_bytes());
        (x.len() as u8, buf)
    }

    pub fn word(x: &str) -> Self {
        Self::Word(Self::to_symbol(x))
    }

    pub fn set_word(x: &str) -> Self {
        Self::SetWord(Self::to_symbol(x))
    }

    pub fn string(x: &str, blobs: &mut impl Storage) -> Self {
        let len = x.len();
        if len <= INLINE_CONTENT_LEN {
            let mut buf = [0u8; INLINE_CONTENT_LEN];
            buf[..len].copy_from_slice(x.as_bytes());
            Value::String(Content::Inline((len as u8, buf)))
        } else {
            Value::String(Content::Hash(blobs.put(x.as_bytes())))
        }
    }

    pub fn block(values: Vec<Value>) -> Self {
        Value::Block(values.into_boxed_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NullStorage;

    impl Storage for NullStorage {
        fn put(&mut self, _data: &[u8]) -> Hash {
            unreachable!()
        }
    }

    #[test]
    fn test_block_builder() {
        let mut blobs = NullStorage;

        let v1 = Value::uint(18);
        let v2 = Value::float(3.14);
        let v3 = Value::string("hello world", &mut blobs);
        let block = Value::block(vec![v1, v2, v3]);
        let block2 = Value::block(vec![
            Value::uint(1000),
            Value::float(2.718),
            Value::string("привет!", &mut blobs),
        ]);

        println!("{:?}", Value::block(vec![block, block2]));
    }
}
