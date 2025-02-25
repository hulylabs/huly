// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use smol_str::SmolStr;
use std::array::TryFromSliceError;
use std::io::IoSlice;
use std::marker::PhantomData;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Blob not found")]
    BlobNotFound,
    #[error("Word not found")]
    WordNotFound,
    #[error(transparent)]
    ArrayError(#[from] TryFromSliceError),
    #[error("Out of bounds")]
    OutOfBounds,
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("Invalid tag")]
    InvalidTag,
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("parse collector error")]
    ParseCollectorError,
    #[error("unexpected character: `{0}`")]
    UnexpectedChar(char),
    #[error("internal error")]
    InternalError,
    #[error("integer overflow")]
    IntegerOverflow,
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("symbol too long")]
    SymbolTooLong,
}

//

pub struct Array<T, I>(T, PhantomData<I>);

impl<T, I> Array<T, I> {
    pub fn new(data: T) -> Self {
        Array(data, PhantomData)
    }
}

impl<T, I> Array<T, I>
where
    T: AsRef<[u8]>,
{
    const INDEX_SIZE: usize = std::mem::size_of::<I>();

    fn get_index(data: &[u8], pos: usize) -> Option<usize> {
        data.get(pos..pos + Self::INDEX_SIZE).map(|bytes| {
            let mut len = 0;
            for byte in bytes {
                len = len << 8 | *byte as usize;
            }
            len
        })
    }

    pub fn get(&self, index: usize) -> Option<Value> {
        let data = self.0.as_ref();
        let len = Self::get_index(data, 0)?;
        if index < len {
            let offset = Self::get_index(data, data.len() - (index + 1) * Self::INDEX_SIZE)?;
            Value::load(&data[offset..])
        } else {
            None
        }
    }
}

//

#[derive(Debug, Clone)]
pub enum WordKind {
    Word,
    SetWord,
}

impl WordKind {
    fn write<W: std::io::Write>(
        self,
        write: &mut W,
        tag: u8,
        symbol: &SmolStr,
    ) -> Result<(), CoreError> {
        if symbol.len() > 0xff {
            return Err(CoreError::SymbolTooLong);
        }
        let bytes = symbol.as_bytes();
        write.write_vectored(&[
            IoSlice::new(&[tag | self as u8, symbol.len() as u8]),
            IoSlice::new(bytes),
        ])?;
        Ok(())
    }

    fn load(data: &[u8]) -> Option<SmolStr> {
        data.split_first().and_then(|(len, data)| {
            let len = *len as usize;
            let bytes = data.get(..len)?;
            std::str::from_utf8(bytes).ok().map(SmolStr::from)
        })
    }
}

//

pub const HASH_SIZE: usize = 32;
pub type Hash = [u8; 32];

#[derive(Clone)]
pub enum Blob {
    Inline(Hash),
    External(Hash),
}

/// Display implementation for Blob provides a concise view, useful for end users
impl std::fmt::Display for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inline(container) => {
                let len = container[0] as usize;
                // For display, just indicate if it's a small or large blob
                if len < 10 {
                    write!(f, "small blob")
                } else {
                    write!(f, "blob ({} bytes)", len)
                }
            },
            Self::External(_) => {
                // For display, we don't show hashes to end users
                write!(f, "large blob")
            }
        }
    }
}

/// Debug implementation for Blob provides technical details, useful for programmers
impl std::fmt::Debug for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inline(container) => {
                let len = container[0] as usize;
                // Show actual data bytes for small inline blobs
                if len < 16 && len > 0 {
                    let data = &container[1..len+1];
                    write!(f, "Inline(size={}, data={:?})", len, data)
                } else {
                    write!(f, "Inline(size={})", len)
                }
            },
            Self::External(hash) => {
                // Format the hash with first few and last few bytes for programmers
                write!(f, "External(hash={:02x}{:02x}..{:02x}{:02x})", 
                      hash[0], hash[1], 
                      hash[hash.len()-2], hash[hash.len()-1])
            }
        }
    }
}

impl Blob {
    const EXTERNAL: u8 = 0x10;

    fn load(options: u8, data: &[u8]) -> Option<Blob> {
        if options == Self::EXTERNAL {
            data.get(..32)?.try_into().ok().map(Blob::External)
        } else {
            data.split_first().and_then(|(len, data)| {
                let len = *len as usize;
                let mut inline = [0u8; HASH_SIZE];
                inline.get_mut(..len).map(|dst| dst.copy_from_slice(data));
                Some(Blob::Inline(inline))
            })
        }
    }

    fn write<W: std::io::Write>(&self, write: &mut W, tag: u8) -> Result<(), CoreError> {
        let _ = match self {
            Blob::Inline(data) => {
                let slice = data
                    .split_first()
                    .and_then(|(len, data)| data.get(..*len as usize))
                    .ok_or(CoreError::OutOfBounds)?;
                write.write_vectored(&[IoSlice::new(&[tag]), IoSlice::new(slice)])?
            }
            Blob::External(hash) => write
                .write_vectored(&[IoSlice::new(&[tag | Self::EXTERNAL]), IoSlice::new(hash)])?,
        };
        Ok(())
    }
}

//

pub trait BlobStore {
    fn get(&self, hash: &Hash) -> Result<&[u8], CoreError>;
    fn put(&mut self, data: &[u8]) -> Result<Hash, CoreError>;

    fn create(&mut self, data: &[u8]) -> Result<Blob, CoreError> {
        let len = data.len();
        if len < HASH_SIZE {
            let mut inline: Hash = [0u8; HASH_SIZE];
            inline
                .split_first_mut()
                .and_then(|(len, dst)| {
                    *len = data.len() as u8;
                    dst.get_mut(..data.len())
                        .map(|dst| dst.copy_from_slice(data))
                })
                .ok_or(CoreError::InternalError)?;
            Ok(Blob::Inline(inline))
        } else {
            self.put(data).map(Blob::External)
        }
    }

    fn get_block_value(&self, blob: &Blob, index: usize) -> Option<Value> {
        match blob {
            Blob::Inline(container) => {
                let data = container
                    .split_first()
                    .and_then(|(len, data)| data.get(..*len as usize))?;
                Array::<&[u8], u8>::new(data).get(index)
            }
            Blob::External(hash) => {
                let data = self.get(hash).ok()?;
                Array::<&[u8], u32>::new(data).get(index)
            }
        }
    }
}

//

#[derive(Clone)]
pub enum Value {
    None,
    Int(i64),
    // Float(f64),
    Block(Blob),
    String(Blob),
    Word(SmolStr),
    SetWord(SmolStr),
}

/// Display format is designed for end users seeing the values in an interpreter.
/// It is more concise and readable, more like what a user would expect to see.
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Int(i) => write!(f, "{}", i),
            Self::Block(blob) => {
                // For blocks, we display a simplified representation
                match blob {
                    Blob::Inline(_) => write!(f, "[...]"),
                    Blob::External(_) => write!(f, "[...]"),
                }
            },
            Self::String(blob) => match blob {
                Blob::Inline(container) => {
                    let len = container[0] as usize;
                    if len == 0 {
                        write!(f, "\"\"")
                    } else {
                        // Extract the string content from the inline container
                        if let Ok(s) = std::str::from_utf8(&container[1..len+1]) {
                            write!(f, "\"{}\"", s)
                        } else {
                            write!(f, "\"<binary data>\"")
                        }
                    }
                },
                Blob::External(_) => {
                    // For external strings, we just indicate it's a string (user doesn't need hash details)
                    write!(f, "\"...\"")
                }
            },
            Self::Word(word) => write!(f, "{}", word),
            Self::SetWord(word) => write!(f, "{}:", word),
        }
    }
}

/// Debug format is designed for programmers and includes more technical details.
impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Int(i) => write!(f, "Int({})", i),
            Self::Block(blob) => match blob {
                Blob::Inline(container) => {
                    let len = container[0] as usize;
                    write!(f, "Block::Inline(size={})", len)
                },
                Blob::External(hash) => {
                    // Format the hash nicely with a prefix for debugging
                    write!(f, "Block::External({:02x}{:02x}..{:02x}{:02x})", 
                        hash[0], hash[1], 
                        hash[hash.len()-2], hash[hash.len()-1])
                }
            },
            Self::String(blob) => match blob {
                Blob::Inline(container) => {
                    let len = container[0] as usize;
                    if len == 0 {
                        write!(f, "String::Inline(\"\")")
                    } else {
                        // Extract the string content from the inline container
                        if let Ok(s) = std::str::from_utf8(&container[1..len+1]) {
                            write!(f, "String::Inline(\"{}\")", s)
                        } else {
                            write!(f, "String::Inline(<binary data>, size={})", len)
                        }
                    }
                },
                Blob::External(hash) => {
                    // Format the hash nicely with a prefix for debugging
                    write!(f, "String::External({:02x}{:02x}..{:02x}{:02x})", 
                        hash[0], hash[1], 
                        hash[hash.len()-2], hash[hash.len()-1])
                }
            },
            Self::Word(word) => write!(f, "Word(\"{}\")", word),
            Self::SetWord(word) => write!(f, "SetWord(\"{}\")", word),
        }
    }
}

impl Value {
    const TAG_INT: u8 = 0;
    const TAG_BLOCK: u8 = 1;
    const TAG_STRING: u8 = 2;
    const TAG_WORD: u8 = 3;
    const TAG_NONE: u8 = 4;

    const OPTION_MASK: u8 = 0xf0;

    fn load(data: &[u8]) -> Option<Self> {
        let header = data.get(0).copied()?;
        let tag = header & 0b111;
        match tag {
            Self::TAG_NONE => Some(Self::None),
            Self::TAG_INT => data.get(1..9).map(|bytes| {
                let mut value = 0;
                for byte in bytes {
                    value = value << 8 | *byte as i64;
                }
                Self::Int(value)
            }),
            Self::TAG_BLOCK => Blob::load(header & Self::OPTION_MASK, &data[1..]).map(Self::Block),
            Self::TAG_STRING => {
                Blob::load(header & Self::OPTION_MASK, &data[1..]).map(Self::String)
            }
            Self::TAG_WORD => WordKind::load(&data[1..]).map(Self::Word),
            _ => None,
        }
    }

    pub fn write<W: std::io::Write>(&self, write: &mut W) -> Result<(), CoreError> {
        match self {
            Self::None => write.write_all(&[Self::TAG_NONE]).map_err(Into::into),
            Self::Int(value) => {
                let mut buf = [0u8; 9];
                buf[0] = Self::TAG_INT;
                for i in 0..8 {
                    buf[8 - i] = (value >> (i * 8)) as u8;
                }
                write.write_all(&buf).map_err(Into::into)
            }
            Self::Block(blob) => blob.write(write, Self::TAG_BLOCK),
            Self::String(blob) => blob.write(write, Self::TAG_STRING),
            Self::Word(word) => WordKind::Word.write(write, Self::TAG_WORD, word),
            Self::SetWord(word) => WordKind::SetWord.write(write, Self::TAG_WORD, word),
        }
    }
}

//

pub fn load_value(data: &[u8]) -> Option<Value> {
    Value::load(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create an inline blob containing a string
    fn create_string_blob(s: &str) -> Blob {
        let bytes = s.as_bytes();
        let len = bytes.len();
        if len >= HASH_SIZE {
            panic!("String too long for inline blob test");
        }
        
        let mut container = [0u8; HASH_SIZE];
        container[0] = len as u8;
        container[1..len+1].copy_from_slice(bytes);
        
        Blob::Inline(container)
    }
    
    /// Helper to create an external blob with a mock hash
    fn create_external_blob() -> Blob {
        let mut hash = [0u8; HASH_SIZE];
        // Create a recognizable pattern in the hash
        for i in 0..HASH_SIZE {
            hash[i] = (i as u8) % 255;
        }
        Blob::External(hash)
    }
    
    #[test]
    fn test_display_basic_values() {
        // Test display for basic values
        assert_eq!(Value::None.to_string(), "none");
        assert_eq!(Value::Int(42).to_string(), "42");
        assert_eq!(Value::Word(SmolStr::new("hello")).to_string(), "hello");
        assert_eq!(Value::SetWord(SmolStr::new("x")).to_string(), "x:");
    }
    
    #[test]
    fn test_display_string_values() {
        // Test display for string values
        let empty_string = Value::String(create_string_blob(""));
        let short_string = Value::String(create_string_blob("hello"));
        let external_string = Value::String(create_external_blob());
        
        // Check Display format for end users
        assert_eq!(empty_string.to_string(), "\"\"");
        assert_eq!(short_string.to_string(), "\"hello\"");
        assert_eq!(external_string.to_string(), "\"...\"");
        
        // Check Debug format for programmers
        assert_eq!(format!("{:?}", empty_string), "String::Inline(\"\")");
        assert_eq!(format!("{:?}", short_string), "String::Inline(\"hello\")");
        assert!(format!("{:?}", external_string).starts_with("String::External("));
    }
    
    #[test]
    fn test_display_block_values() {
        // Test display for block values
        let inline_block = Value::Block(create_string_blob("test"));
        let external_block = Value::Block(create_external_blob());
        
        // Check Display format for end users
        assert_eq!(inline_block.to_string(), "[...]");
        assert_eq!(external_block.to_string(), "[...]");
        
        // Check Debug format for programmers
        assert!(format!("{:?}", inline_block).starts_with("Block::Inline"));
        assert!(format!("{:?}", external_block).starts_with("Block::External"));
    }
    
    #[test]
    fn test_blob_display_and_debug() {
        // Test Blob Display and Debug formats
        let inline_blob = create_string_blob("abc");
        let external_blob = create_external_blob();
        
        // Display for end users should be simple
        assert_eq!(inline_blob.to_string(), "small blob");
        assert_eq!(external_blob.to_string(), "large blob");
        
        // Debug for programmers should be detailed
        let debug_inline = format!("{:?}", inline_blob);
        let debug_external = format!("{:?}", external_blob);
        
        assert!(debug_inline.contains("size=3"));
        assert!(debug_inline.contains("data="));
        assert!(debug_external.contains("External(hash="));
        assert!(debug_external.contains(".."));
    }
}
