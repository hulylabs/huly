// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// value.rs:

use crate::heap::Heap;
use std::fmt;
use std::io::{self, Write};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValueError {
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error("index out of bounds {0} [0..{1}]")]
    OutOfBounds(usize, usize),
}

pub type Result<T> = std::result::Result<T, ValueError>;

pub trait Serialize {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()>;
}

pub trait Deserialize: Sized {
    fn deserialize(bytes: &[u8]) -> Result<(Self, usize)>;
}

// should we stick to 32 + 4 - 2 bytes for better support of 32-bit systems?
const INLINE_CONTENT_BUFFER: usize = 32 + 8 - 2;

pub const CONTENT_TYPE_UNKNOWN: u8 = 0x00;
const CONTENT_TYPE_UTF8: u8 = 0x01;

#[derive(Clone)]
pub enum Value {
    None,

    // Following types directly map to Wasm value types
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),

    Bytes(u8, Content),

    Hash([u8; 32]),
    PubKey([u8; 32]),

    Word(Symbol),
    SetWord(Symbol),

    Block(Content),
    Context(Content),

    NativeFn(usize, usize),
}

impl Value {
    pub fn none() -> Self {
        Self::None
    }

    pub fn string(str: &str, heap: &mut impl Heap) -> Self {
        Self::Bytes(CONTENT_TYPE_UTF8, Content::new(str.as_bytes(), heap))
    }

    pub fn word(str: &str) -> Result<Self> {
        Ok(Self::Word(Symbol::new(str)?))
    }

    pub fn set_word(str: &str) -> Result<Self> {
        Ok(Self::SetWord(Symbol::new(str)?))
    }

    pub fn block(block: &[Value], heap: &mut impl Heap) -> Result<Self> {
        let mut bytes = Vec::new();
        for value in block {
            value.serialize(&mut bytes)?;
        }
        Ok(Self::Block(Content::new(&bytes, heap)))
    }

    pub fn context(context: &[(Symbol, Value)], heap: &mut impl Heap) -> Result<Self> {
        let mut bytes = Vec::new();
        for value in context {
            value.0.serialize(&mut bytes)?;
            value.1.serialize(&mut bytes)?;
        }
        Ok(Self::Context(Content::new(&bytes, heap)))
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::I32(x) => Some(*x as i64),
            Value::I64(x) => Some(*x),
            _ => None,
        }
    }

    /// Attempts to extract a string representation of the value
    ///
    /// # Returns
    /// - `Some(&str)`: Always returns symbol for all word variants
    /// - `Some(&str)`: For `Bytes` which are `Utf8` encoded if content fits in the inline buffer
    /// - `None`: For all other variants
    ///
    /// # Safety
    /// This method is safe because:
    /// - The serialization format preserves `Utf8` encoding
    /// - The data is immutable after deserialization
    pub unsafe fn inlined_as_str(&self) -> Option<&str> {
        match self {
            Value::Bytes(CONTENT_TYPE_UTF8, content) => content
                .inlined()
                .map(|bytes| std::str::from_utf8_unchecked(bytes)),
            Value::Word(symbol) => Some(symbol.symbol()),
            Value::SetWord(symbol) => Some(symbol.symbol()),
            _ => None,
        }
    }
}

const TAG_NONE: u8 = 0x00;
const TAG_I32: u8 = 0x01;
const TAG_I64: u8 = 0x02;
const TAG_F32: u8 = 0x03;
const TAG_F64: u8 = 0x08;
const TAG_BYTES: u8 = 0x04;
const TAG_WORD: u8 = 0x05;
const TAG_SET_WORD: u8 = 0x06;
const TAG_BLOCK: u8 = 0x07;

impl Serialize for Value {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            Value::None => writer.write_all(&[TAG_NONE])?,
            Value::I32(x) => {
                writer.write_all(&[TAG_I32])?;
                writer.write_all(&x.to_le_bytes())?;
            }
            Value::I64(x) => {
                writer.write_all(&[TAG_I64])?;
                writer.write_all(&x.to_le_bytes())?;
            }
            Value::F32(x) => {
                writer.write_all(&[TAG_F32])?;
                writer.write_all(&x.to_le_bytes())?;
            }
            Value::F64(x) => {
                writer.write_all(&[TAG_F64])?;
                writer.write_all(&x.to_le_bytes())?;
            }
            Value::Bytes(enc, content) => {
                writer.write_all(&[TAG_BYTES, *enc])?;
                content.serialize(writer)?;
            }
            Value::Word(x) => {
                writer.write_all(&[TAG_WORD])?;
                x.serialize(writer)?;
            }
            Value::SetWord(x) => {
                writer.write_all(&[TAG_SET_WORD])?;
                x.serialize(writer)?;
            }
            Value::Block(x) => {
                writer.write_all(&[TAG_BLOCK])?;
                x.serialize(writer)?;
            }
            _ => unimplemented!(),
        }
        Ok(())
    }
}

impl Deserialize for Value {
    fn deserialize(bytes: &[u8]) -> Result<(Self, usize)> {
        if bytes.is_empty() {
            return Err(ValueError::OutOfBounds(0, 1));
        }
        let tag = bytes[0];
        match tag {
            TAG_NONE => Ok((Value::None, 1)),
            TAG_I32 => {
                const LEN: usize = std::mem::size_of::<i32>() + 1;
                if bytes.len() < LEN {
                    return Err(ValueError::OutOfBounds(LEN, bytes.len()));
                }
                let mut buf = [0u8; LEN - 1];
                buf.copy_from_slice(&bytes[1..LEN]);
                Ok((Value::I32(i32::from_le_bytes(buf)), LEN))
            }
            TAG_I64 => {
                const LEN: usize = std::mem::size_of::<i64>() + 1;
                if bytes.len() < LEN {
                    return Err(ValueError::OutOfBounds(LEN, bytes.len()));
                }
                let mut buf = [0u8; LEN - 1];
                buf.copy_from_slice(&bytes[1..LEN]);
                Ok((Value::I64(i64::from_le_bytes(buf)), LEN))
            }
            TAG_F32 => {
                const LEN: usize = std::mem::size_of::<f32>() + 1;
                if bytes.len() < LEN {
                    return Err(ValueError::OutOfBounds(LEN, bytes.len()));
                }
                let mut buf = [0u8; LEN - 1];
                buf.copy_from_slice(&bytes[1..LEN]);
                Ok((Value::F32(f32::from_le_bytes(buf)), LEN))
            }
            TAG_F64 => {
                const LEN: usize = std::mem::size_of::<f64>() + 1;
                if bytes.len() < LEN {
                    return Err(ValueError::OutOfBounds(LEN, bytes.len()));
                }
                let mut buf = [0u8; LEN - 1];
                buf.copy_from_slice(&bytes[1..LEN]);
                Ok((Value::F64(f64::from_le_bytes(buf)), LEN))
            }
            TAG_BYTES => {
                if bytes.len() < 2 {
                    return Err(ValueError::OutOfBounds(2, bytes.len()));
                }
                let enc = bytes[1];
                let (content, len) = Content::deserialize(&bytes[2..])?;
                Ok((Value::Bytes(enc, content), len + 2))
            }
            TAG_WORD => {
                let (symbol, len) = Symbol::deserialize(&bytes[1..])?;
                Ok((Value::Word(symbol), len + 1))
            }
            TAG_SET_WORD => {
                let (symbol, len) = Symbol::deserialize(&bytes[1..])?;
                Ok((Value::SetWord(symbol), len + 1))
            }
            TAG_BLOCK => {
                let (content, len) = Content::deserialize(&bytes[1..])?;
                Ok((Value::Block(content), len + 1))
            }
            _ => unimplemented!(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::None => write!(f, "None"),
            Value::I32(x) => write!(f, "{}", x),
            Value::I64(x) => write!(f, "{}", x),
            Value::F32(x) => write!(f, "{}", x),
            Value::F64(x) => write!(f, "{}", x),
            Value::Bytes(CONTENT_TYPE_UTF8, _) => {
                write!(f, "{}", unsafe { self.inlined_as_str().unwrap() })
            }
            Value::Word(x) => write!(f, "{}", unsafe { x.symbol() }),
            Value::SetWord(x) => write!(f, "{}:", unsafe { x.symbol() }),
            Value::Block(_) => write!(f, "Block(...)"),
            Value::NativeFn(module, proc) => {
                write!(f, "native proc: module {}, proc {}", module, proc)
            }
            _ => unimplemented!(),
        }
    }
}

// C O N T E N T

#[derive(Clone)]
pub struct Content {
    content: [u8; INLINE_CONTENT_BUFFER],
}

impl Content {
    pub fn new(content: &[u8], heap: &mut impl Heap) -> Self {
        let len = content.len();
        if len < INLINE_CONTENT_BUFFER {
            let mut buffer = [0u8; INLINE_CONTENT_BUFFER];
            buffer[0] = len as u8;
            buffer[1..len + 1].copy_from_slice(&content[..len]);
            Self { content: buffer }
        } else {
            let hash = heap.put(content);
            let mut buffer = [0u8; INLINE_CONTENT_BUFFER];
            buffer[0] = 0xff;
            buffer[1..33].copy_from_slice(&hash);
            Self { content: buffer }
        }
    }

    fn inlined(&self) -> Option<&[u8]> {
        let len = self.content[0] as usize;
        if len < INLINE_CONTENT_BUFFER {
            Some(&self.content[1..len + 1])
        } else {
            None
        }
    }
}

impl Serialize for Content {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let len = self.content[0] as usize;
        let len = if len < INLINE_CONTENT_BUFFER { len } else { 32 };
        writer.write_all(&self.content[0..len + 1])?;
        Ok(())
    }
}

impl Deserialize for Content {
    fn deserialize(bytes: &[u8]) -> Result<(Self, usize)> {
        let mut content = [0u8; INLINE_CONTENT_BUFFER];
        let len = bytes
            .first()
            .copied()
            .ok_or(ValueError::OutOfBounds(0, 1))? as usize;
        let len = if len < INLINE_CONTENT_BUFFER { len } else { 32 };
        if bytes.len() < len + 1 {
            return Err(ValueError::OutOfBounds(len + 1, bytes.len()));
        }
        content[..len + 1].copy_from_slice(&bytes[..len + 1]);
        Ok((Self { content }, len + 1))
    }
}

impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // dump content in hexdump format with ascii representation
        // Hexdump-like format, 16 bytes per line
        for chunk in self.content.to_vec().chunks(16) {
            write!(f, "      ")?;
            for b in chunk {
                write!(f, "{:02x} ", b)?;
            }
            for _ in chunk.len()..16 {
                write!(f, "   ")?;
            }
            write!(f, " |")?;
            for &b in chunk {
                let c = if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                };
                write!(f, "{}", c)?;
            }
            for _ in chunk.len()..16 {
                write!(f, " ")?;
            }
            writeln!(f, "|")?;
        }
        writeln!(f)?;
        Ok(())
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::None => write!(f, "None"),
            Value::I32(x) => write!(f, "{}", x),
            Value::I64(x) => write!(f, "{}", x),
            Value::F32(x) => write!(f, "{}", x),
            Value::F64(x) => write!(f, "{}", x),
            Value::Bytes(enc, content) => {
                writeln!(f, "Bytes ({:02x})", enc)?;
                writeln!(f, "{:?}", content)
            }
            Value::Hash(hash) => write!(f, "Hash({})", hex::encode(hash)),
            Value::PubKey(hash) => write!(f, "PubKey({})", hex::encode(hash)),
            Value::Word(symbol) => write!(f, "Word({})", unsafe { symbol.symbol() }),
            Value::SetWord(symbol) => write!(f, "SetWord({})", unsafe { symbol.symbol() }),
            Value::Block(content) => write!(f, "Block({:?})", content),
            Value::Context(content) => write!(f, "Context({:?})", content),
            Value::NativeFn(module, proc) => {
                write!(f, "NativeFn(module: {}, proc: {})", module, proc)
            }
        }
    }
}

// S Y M B O L

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    symbol: [u8; INLINE_CONTENT_BUFFER],
}

impl Symbol {
    pub fn new(content: &str) -> Result<Self> {
        let len = content.len();
        if len < INLINE_CONTENT_BUFFER {
            let mut symbol = [0u8; INLINE_CONTENT_BUFFER];
            symbol[0] = len as u8;
            symbol[1..len + 1].copy_from_slice(&content.as_bytes()[..len]);
            Ok(Self { symbol })
        } else {
            Err(ValueError::OutOfBounds(len, INLINE_CONTENT_BUFFER - 1))
        }
    }

    unsafe fn symbol(&self) -> &str {
        let len = self.symbol[0] as usize;
        std::str::from_utf8_unchecked(&self.symbol[1..len + 1])
    }
}

impl Serialize for Symbol {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let len = self.symbol[0] as usize;
        writer.write_all(&self.symbol[0..len + 1])?;
        Ok(())
    }
}

impl Deserialize for Symbol {
    fn deserialize(bytes: &[u8]) -> Result<(Self, usize)> {
        let mut symbol = [0u8; INLINE_CONTENT_BUFFER];
        let len = bytes
            .first()
            .copied()
            .ok_or(ValueError::OutOfBounds(0, 1))? as usize;
        if bytes.len() < len + 1 {
            return Err(ValueError::OutOfBounds(len + 1, bytes.len()));
        }
        symbol[..len + 1].copy_from_slice(&bytes[..len + 1]);
        Ok((Self { symbol }, len + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content() {
        let mut heap = crate::heap::TempHeap::new();
        let content = Content::new(b"hello", &mut heap);
        assert_eq!(content.content[0], 5);
        assert_eq!(&content.content[1..6], b"hello");
        let (deserialized, _) = Content::deserialize(&content.content).unwrap();
        assert_eq!(content.content, deserialized.content);
    }

    #[test]
    fn test_symbol() {
        let symbol = Symbol::new("hello").unwrap();
        assert_eq!(symbol.symbol[0], 5);
        assert_eq!(&symbol.symbol[1..6], b"hello");
        let (deserialized, _) = Symbol::deserialize(&symbol.symbol).unwrap();
        assert_eq!(symbol.symbol, deserialized.symbol);
    }

    #[test]
    fn test_context() -> Result<()> {
        let mut heap = crate::heap::TempHeap::new();
        let kv = vec![
            (Symbol::new("hello")?, Value::I32(42)),
            (Symbol::new("there")?, Value::I64(12341234)),
            (Symbol::new("how")?, Value::I32(12341234)),
            (Symbol::new("doing")?, Value::I64(12341234)),
        ];
        let ctx = Value::context(&kv, &mut heap)?;
        match ctx {
            Value::Context(content) => {
                println!("{:?}", content);
            }
            _ => panic!("expected Value::Context"),
        }
        Ok(())
    }

    #[test]
    fn test_value() -> Result<()> {
        let mut heap = crate::heap::TempHeap::new();
        let value = Value::string("hello", &mut heap);
        let mut bytes = Vec::new();
        value.serialize(&mut bytes)?;
        let (deserialized, _) = Value::deserialize(&bytes).unwrap();
        unsafe {
            assert_eq!(deserialized.inlined_as_str(), Some("hello"));
        }
        Ok(())
    }

    #[test]
    fn test_string() -> Result<()> {
        let mut heap = crate::heap::TempHeap::new();
        let value = Value::string("hello, world!", &mut heap);
        let mut bytes = Vec::new();
        value.serialize(&mut bytes)?;
        let (deserialized, _) = Value::deserialize(&bytes).unwrap();
        unsafe {
            assert_eq!(deserialized.inlined_as_str(), Some("hello, world!"));
        }
        println!("{:?}", deserialized);
        Ok(())
    }
}
