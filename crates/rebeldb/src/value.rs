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

// should we stick to 32 + 4 - 1 bytes for better support of 32-bit systems?
const INLINE_CONTENT_BUFFER: usize = 32 + 8 - 1;

#[derive(Debug, Clone)]
pub enum Value {
    None,

    Uint(u32),
    Int(i32),
    Float(f32),

    Bytes(Content),
    String(Content),

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

    pub fn uint(x: u32) -> Self {
        Self::Uint(x)
    }

    pub fn int(x: i32) -> Self {
        Self::Int(x)
    }

    pub fn float(x: f32) -> Self {
        Self::Float(x)
    }

    pub fn string(str: &str, heap: &mut impl Heap) -> Self {
        Self::String(Content::new(str.as_bytes(), heap))
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
            Value::Uint(x) => Some(*x as i64),
            Value::Int(x) => Some(*x as i64),
            _ => None,
        }
    }

    /// Attempts to extract a string from the inline buffer.
    ///
    /// # Returns
    /// - `Some(&str)`: Always returns a string for word variants
    /// - `Some(&str)`: For `String` variant if content fits in the inline buffer
    /// - `None`: For `String` variant if content exceeds inline buffer capacity
    /// - `None`: For all other variants
    ///
    /// # Safety
    /// This method is safe because:
    /// - All bytes are validated as UTF-8 during value creation
    /// - The serialization format preserves UTF-8 encoding
    /// - The byte buffer is immutable after deserialization
    pub unsafe fn inlined_as_str(&self) -> Option<&str> {
        match self {
            Value::String(content) => content.inlined_as_str(),
            Value::Word(symbol) => Some(symbol.symbol()),
            Value::SetWord(symbol) => Some(symbol.symbol()),
            _ => None,
        }
    }
}

const TAG_NONE: u8 = 0x00;
const TAG_UINT: u8 = 0x01;
const TAG_INT: u8 = 0x02;
const TAG_FLOAT: u8 = 0x03;
const TAG_STRING: u8 = 0x04;
const TAG_WORD: u8 = 0x05;
const TAG_SET_WORD: u8 = 0x06;
const TAG_BLOCK: u8 = 0x07;

impl Serialize for Value {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            Value::None => writer.write_all(&[TAG_NONE])?,
            Value::Uint(x) => {
                writer.write_all(&[TAG_UINT])?;
                writer.write_all(&x.to_le_bytes())?;
            }
            Value::Int(x) => {
                writer.write_all(&[TAG_INT])?;
                writer.write_all(&x.to_le_bytes())?;
            }
            Value::Float(x) => {
                writer.write_all(&[TAG_FLOAT])?;
                writer.write_all(&x.to_le_bytes())?;
            }
            Value::String(x) => {
                writer.write_all(&[TAG_STRING])?;
                x.serialize(writer)?;
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
            TAG_UINT => {
                if bytes.len() < 5 {
                    return Err(ValueError::OutOfBounds(5, bytes.len()));
                }
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&bytes[1..5]);
                Ok((Value::Uint(u32::from_le_bytes(buf)), 5))
            }
            TAG_INT => {
                if bytes.len() < 5 {
                    return Err(ValueError::OutOfBounds(5, bytes.len()));
                }
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&bytes[1..5]);
                Ok((Value::Int(i32::from_le_bytes(buf)), 5))
            }
            TAG_FLOAT => {
                if bytes.len() < 5 {
                    return Err(ValueError::OutOfBounds(5, bytes.len()));
                }
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&bytes[1..5]);
                Ok((Value::Float(f32::from_le_bytes(buf)), 5))
            }
            TAG_STRING => {
                let (content, len) = Content::deserialize(&bytes[1..])?;
                Ok((Value::String(content), len + 1))
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
            Value::Uint(x) => write!(f, "{}", x),
            Value::Int(x) => write!(f, "{}", x),
            Value::Float(x) => write!(f, "{}", x),
            Value::String(x) => write!(f, "{}", unsafe { x.inlined_as_str().unwrap() }),
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

    unsafe fn inlined_as_str(&self) -> Option<&str> {
        let len = self.content[0] as usize;
        if len < INLINE_CONTENT_BUFFER {
            Some(std::str::from_utf8_unchecked(&self.content[1..len + 1]))
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
            (Symbol::new("hello")?, Value::uint(42)),
            (Symbol::new("there")?, Value::uint(12341234)),
            (Symbol::new("how")?, Value::uint(12341234)),
            (Symbol::new("doing")?, Value::uint(12341234)),
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
}
