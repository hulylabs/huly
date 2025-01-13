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
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<usize>;
}

pub trait Deserialize: Sized {
    fn deserialize(bytes: &[u8]) -> Result<Self>;
}

// should we stick to 32 + 4 - 2 bytes for better support of 32-bit systems?
const INLINE_CONTENT_BUFFER: usize = 32 + 8 - 2;

pub const CONTENT_TYPE_UNKNOWN: u8 = 0x00;
const CONTENT_TYPE_UTF8: u8 = 0x01;

pub enum ValueType {
    None,
    Int,
    Float,
    Bytes,
    Hash,
    PubKey,
    Word,
    SetWord,
    Block,
    Context,
}

#[derive(Clone)]
pub enum Value {
    None,

    // Following types directly map to Wasm value types
    Int(i64),
    Float(f64),

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
            Value::Int(x) => Some(*x),
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

//

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxedValue(u64);

/// We force the exponent bits (62..52) to 0x7FF, ensuring it's a NaN if fraction != 0.
/// sign bit (63) is free for marking negative vs. positive integers.
/// fraction (52 bits): top 4 bits (51..48) are the tag, lower 48 bits (47..0) are payload.
const EXP_SHIFT: u64 = 52;
const EXP_MAX: u64 = 0x7FF; // exponent bits all 1 => 0x7FF
const EXP_MASK: u64 = EXP_MAX << EXP_SHIFT; // bits 62..52

/// Bit positions for tag and sign
const TAG_SHIFT: u64 = 48; // so bits 51..48 are the tag
const TAG_MASK: u64 = 0xF; // 4 bits

/// In this layout:
///  bit 63 = sign
///  bits 62..52 = exponent = 0x7FF
///  bits 51..48 = tag
///  bits 47..0  = payload

/// Example tags:
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Int = 0x0, // up to you which nibble you choose
    Ptr = 0x1,
    // You can define up to 16 different tags (0x0 .. 0xF).
}

impl BoxedValue {
    /// Create a boxed *signed* integer.
    /// Uses the top (bit 63) as the "sign bit" for negative vs. non-negative.
    /// The integer's absolute value must fit in 48 bits: -(2^47) .. 2^47 - 1.
    pub fn new_int(value: i64) -> Self {
        // sign bit: 1 if negative, 0 otherwise
        let sign_bit = if value < 0 { 1 } else { 0 };
        let mag = value.unsigned_abs(); // absolute value as u64

        // Ensure fits in 48 bits
        assert!(
            mag < (1u64 << 48),
            "Integer out of range for 48-bit magnitude"
        );

        // fraction = [ tag(4 bits) | payload(48 bits) ]
        // top 4 bits of fraction => Tag::Int
        // lower 48 bits => magnitude
        let fraction = ((Tag::Int as u64) & TAG_MASK) << TAG_SHIFT | (mag & 0xFFFF_FFFF_FFFF);

        // Combine sign, exponent=0x7FF, fraction
        let bits = (sign_bit << 63) | EXP_MASK | fraction;
        BoxedValue(bits)
    }

    /// Try to interpret this BoxedValue as an integer.
    pub fn as_int(&self) -> i64 {
        let sign_bit = (self.0 >> 63) & 1;
        // Check exponent == 0x7FF
        let exponent = (self.0 >> EXP_SHIFT) & 0x7FF;
        // Check tag
        let tag = (self.0 >> TAG_SHIFT) & TAG_MASK;
        // Check fraction != 0 => must be a NaN, not Inf
        let fraction = self.0 & ((1u64 << TAG_SHIFT) - 1u64 | (TAG_MASK << TAG_SHIFT));

        // Validate that it *looks* like a NaN-boxed integer
        assert_eq!(exponent, EXP_MAX, "Not a NaN exponent");
        assert_ne!(fraction, 0, "Looks like Infinity, not NaN");
        assert_eq!(tag, Tag::Int as u64, "Not an Int tag");

        // Lower 48 bits = magnitude
        let mag = self.0 & 0x000F_FFFF_FFFF_FFFF; // mask out exponent & sign & top 4 bits
        let magnitude_48 = mag & 0xFFFF_FFFF_FFFF; // bits 47..0

        if sign_bit == 0 {
            // positive or zero
            magnitude_48 as i64
        } else {
            // negative
            -(magnitude_48 as i64)
        }
    }

    /// Create a boxed pointer (for 32-bit addresses).
    /// Tag = Tag::Ptr, exponent=0x7FF, sign=0, fraction bits 47..0 store the pointer.
    pub fn new_ptr(addr: u32) -> Self {
        // If you need more than 32 bits, store additional bits as needed.
        let fraction = ((Tag::Ptr as u64) & TAG_MASK) << TAG_SHIFT | (addr as u64);
        let bits = (0 << 63) // sign = 0
            | EXP_MASK
            | fraction;
        BoxedValue(bits)
    }

    /// Try to interpret this BoxedValue as a 32-bit pointer.
    pub fn as_ptr(&self) -> u32 {
        let exponent = (self.0 >> EXP_SHIFT) & 0x7FF;
        let tag = (self.0 >> TAG_SHIFT) & TAG_MASK;
        let fraction = self.0 & 0x000F_FFFF_FFFF_FFFF;

        // Validate
        assert_eq!(exponent, EXP_MAX, "Not a NaN exponent");
        assert_ne!(fraction, 0, "Looks like Infinity, not NaN");
        assert_eq!(tag, Tag::Ptr as u64, "Not a Ptr tag");

        // Just grab the lower 32 bits
        (fraction & 0xFFFF_FFFF) as u32
    }

    /// Returns the raw bits for debugging or advanced use
    pub fn bits(&self) -> u64 {
        self.0
    }
}

//

const TAG_NONE: u8 = 0x00;
const TAG_INT: u8 = 0x01;
const TAG_FLOAT: u8 = 0x02;
const TAG_BYTES: u8 = 0x04;
const TAG_WORD: u8 = 0x05;
const TAG_SET_WORD: u8 = 0x06;
const TAG_BLOCK: u8 = 0x07;

fn write_slices<W: Write>(writer: &mut W, slices: &[&[u8]]) -> Result<usize> {
    let mut total_size = 0;
    for slice in slices {
        writer.write_all(slice)?;
        total_size += slice.len();
    }
    Ok(total_size)
}

fn write_tag<W: Write>(writer: &mut W, tag: u8) -> Result<usize> {
    write_slices(writer, &[&[tag]])
}

fn write_tag_slice<W: Write>(writer: &mut W, tag: u8, slice: &[u8]) -> Result<usize> {
    write_slices(writer, &[&[tag], slice])
}

fn write_word<W: Write>(writer: &mut W, tag: u8, symbol: &Symbol) -> Result<usize> {
    let tag_size = write_tag(writer, tag)?;
    let symbol_size = symbol.serialize(writer)?;
    Ok(tag_size + symbol_size)
}

impl Serialize for Value {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<usize> {
        match self {
            Value::None => write_tag(writer, TAG_NONE),
            Value::Int(x) => write_tag_slice(writer, TAG_INT, &x.to_le_bytes()),
            Value::Float(x) => write_tag_slice(writer, TAG_FLOAT, &x.to_le_bytes()),
            Value::Bytes(enc, content) => {
                writer.write_all(&[TAG_BYTES, *enc])?;
                let size = content.serialize(writer)?;
                Ok(size + 2)
            }
            Value::Word(x) => write_word(writer, TAG_WORD, x),
            Value::SetWord(x) => write_word(writer, TAG_SET_WORD, x),
            Value::Block(content) => {
                writer.write_all(&[TAG_BLOCK])?;
                let size = content.serialize(writer)?;
                Ok(size + 1)
            }
            _ => unimplemented!(),
        }
    }
}

macro_rules! read_numeric_value {
    ($bytes:expr, $type:ty, $constructor:expr) => {{
        const LEN: usize = std::mem::size_of::<$type>() + 1;
        if $bytes.len() < LEN {
            return Err(ValueError::OutOfBounds(LEN, $bytes.len()));
        }
        let mut buf = [0u8; std::mem::size_of::<$type>()];
        buf.copy_from_slice(&$bytes[1..LEN]);
        Ok($constructor(<$type>::from_le_bytes(buf)))
    }};
}

macro_rules! read_word {
    ($bytes:expr, $constructor:expr) => {{
        let symbol = Symbol::deserialize(&$bytes[1..])?;
        Ok($constructor(symbol))
    }};
}

impl Deserialize for Value {
    fn deserialize(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(ValueError::OutOfBounds(0, 1));
        }
        let tag = bytes[0];
        match tag {
            TAG_NONE => Ok(Value::None),
            TAG_INT => read_numeric_value!(bytes, i64, Value::Int),
            TAG_FLOAT => read_numeric_value!(bytes, f64, Value::Float),
            TAG_WORD => read_word!(bytes, Value::Word),
            TAG_SET_WORD => read_word!(bytes, Value::SetWord),
            TAG_BYTES => {
                if bytes.len() < 2 {
                    return Err(ValueError::OutOfBounds(2, bytes.len()));
                }
                let enc = bytes[1];
                let content = Content::deserialize(&bytes[2..])?;
                Ok(Value::Bytes(enc, content))
            }
            TAG_BLOCK => {
                let content = Content::deserialize(&bytes[1..])?;
                Ok(Value::Block(content))
            }
            _ => unimplemented!(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::None => write!(f, "None"),
            Value::Int(x) => write!(f, "{}", x),
            Value::Float(x) => write!(f, "{}", x),
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
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<usize> {
        let len = self.content[0] as usize;
        let len = if len < INLINE_CONTENT_BUFFER { len } else { 32 };
        writer.write_all(&self.content[0..len + 1])?;
        Ok(len + 1)
    }
}

impl Deserialize for Content {
    fn deserialize(bytes: &[u8]) -> Result<Self> {
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
        Ok(Self { content })
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
            Value::Int(x) => write!(f, "{}", x),
            Value::Float(x) => write!(f, "{}", x),
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
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<usize> {
        let len = self.symbol[0] as usize;
        writer.write_all(&self.symbol[0..len + 1])?;
        Ok(len + 1)
    }
}

impl Deserialize for Symbol {
    fn deserialize(bytes: &[u8]) -> Result<Self> {
        let mut symbol = [0u8; INLINE_CONTENT_BUFFER];
        let len = bytes
            .first()
            .copied()
            .ok_or(ValueError::OutOfBounds(0, 1))? as usize;
        if bytes.len() < len + 1 {
            return Err(ValueError::OutOfBounds(len + 1, bytes.len()));
        }
        symbol[..len + 1].copy_from_slice(&bytes[..len + 1]);
        Ok(Self { symbol })
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
        let deserialized = Content::deserialize(&content.content).unwrap();
        assert_eq!(content.content, deserialized.content);
    }

    #[test]
    fn test_symbol() {
        let symbol = Symbol::new("hello").unwrap();
        assert_eq!(symbol.symbol[0], 5);
        assert_eq!(&symbol.symbol[1..6], b"hello");
        let deserialized = Symbol::deserialize(&symbol.symbol).unwrap();
        assert_eq!(symbol.symbol, deserialized.symbol);
    }

    #[test]
    fn test_context() -> Result<()> {
        let mut heap = crate::heap::TempHeap::new();
        let kv = vec![
            (Symbol::new("hello")?, Value::Int(42)),
            (Symbol::new("there")?, Value::Float(12341234.55)),
            (Symbol::new("how")?, Value::Int(12341234)),
            (Symbol::new("doing")?, Value::Float(1.12341234)),
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
        let deserialized = Value::deserialize(&bytes).unwrap();
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
        let deserialized = Value::deserialize(&bytes).unwrap();
        unsafe {
            assert_eq!(deserialized.inlined_as_str(), Some("hello, world!"));
        }
        println!("{:?}", deserialized);
        Ok(())
    }

    // B O X E D   V A L U E

    // #[test]
    // fn test_int_round_trip() {
    //     // Some sample values that fit in 48 bits
    //     let cases = [
    //         0,
    //         42,
    //         -42,
    //         123_456_789,
    //         -123_456_789,
    //         (1 << 47) - 1,  // 140,737,488,355,327
    //         -(1 << 47) + 1, // -140,737,488,355,327
    //     ];

    //     for &val in &cases {
    //         let boxed = BoxedValue::new_int(val);
    //         let unboxed = boxed.as_int();
    //         assert_eq!(
    //             unboxed, val,
    //             "Failed round-trip for {} => {:?} => {}",
    //             val, boxed, unboxed
    //         );
    //     }
    // }

    // #[test]
    // #[should_panic(expected = "out of range")]
    // fn test_int_overflow() {
    //     // 2^47 is out of range
    //     let _ = BoxedValue::new_int(1 << 47);
    // }

    // #[test]
    // fn test_ptr_round_trip() {
    //     let ptrs = [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF];

    //     for &p in &ptrs {
    //         let boxed = BoxedValue::new_ptr(p);
    //         let unboxed = boxed.as_ptr();
    //         assert_eq!(
    //             unboxed, p,
    //             "Failed round-trip for pointer {:08X} => {:?} => {:08X}",
    //             p, boxed, unboxed
    //         );
    //     }
    // }

    // #[test]
    // fn test_bits_debug() {
    //     let x = BoxedValue::new_int(42);
    //     println!("Boxed bits for 42: 0x{:016X}", x.bits());
    // }
}
