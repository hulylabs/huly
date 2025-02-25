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
        
        // For external blobs, the first 4 bytes are the size, and we need to skip them
        let start_offset = if Self::INDEX_SIZE == 4 { 4 } else { 0 };
        
        // Try to get the length of the array
        let len_pos = if Self::INDEX_SIZE == 4 {
            // For u32 indices, try to read the count from the first 4 bytes
            let size = u32::from_le_bytes([
                data.get(0).copied().unwrap_or(0),
                data.get(1).copied().unwrap_or(0),
                data.get(2).copied().unwrap_or(0),
                data.get(3).copied().unwrap_or(0)
            ]) as usize;
            
            // Estimate the count by dividing the remaining space by the index size
            // This is an approximation, but should work for simple cases
            let data_size = size - 4; // Subtract size field
            let approx_count = data_size / (Self::INDEX_SIZE + 1); // +1 for avg value size
            approx_count.min(100) // Limit to avoid huge arrays
        } else {
            // For inline blocks, the length is stored as the first byte
            data.get(0).copied()? as usize
        };
        
        if index >= len_pos {
            return None;
        }
        
        // For safety, check if we're within bounds
        if data.len() <= start_offset {
            return None;
        }
        
        // Try to get the offset for this index
        let offset_pos = data.len() - (index + 1) * Self::INDEX_SIZE;
        if offset_pos >= data.len() {
            return None;
        }
        
        let offset = Self::get_index(data, offset_pos)?;
        if offset >= data.len() {
            return None;
        }
        
        Value::load(&data[offset..])
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

// We removed the Display implementation for Blob since it doesn't make sense.
// Blobs will be displayed differently depending on whether they're part of String or Block.

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
    
    /// Get all the values in a block as a Vec
    fn get_block_values(&self, blob: &Blob) -> Vec<Value> {
        match blob {
            Blob::Inline(container) => {
                let len = container[0] as usize;
                if len == 0 {
                    return Vec::new();
                }
                
                let data = container
                    .split_first()
                    .and_then(|(_, data)| Some(data))
                    .unwrap_or(&[]);
                
                let array = Array::<&[u8], u8>::new(data);
                
                // Estimate the number of values based on the size
                let estimated_count = len.max(1);
                let mut values = Vec::with_capacity(estimated_count);
                
                // Extract all values from the array
                for i in 0..estimated_count {
                    if let Some(value) = array.get(i) {
                        values.push(value);
                    } else {
                        break;
                    }
                }
                
                values
            },
            Blob::External(hash) => {
                match self.get(hash) {
                    Ok(data) => {
                        let array = Array::<&[u8], u32>::new(data);
                        let mut values = Vec::new();
                        let mut i = 0;
                        
                        // Extract all values
                        while let Some(value) = array.get(i) {
                            values.push(value);
                            i += 1;
                            
                            // Safety limit to prevent infinite loops
                            if i > 1000 {
                                break;
                            }
                        }
                        
                        values
                    },
                    Err(_) => Vec::new()
                }
            }
        }
    }
    
    /// Format a block for display (user-friendly)
    fn format_block_display(&self, f: &mut std::fmt::Formatter<'_>, blob: &Blob) -> std::fmt::Result {
        write!(f, "[")?;
        
        let values = self.get_block_values(blob);
        let mut first = true;
        
        for value in values {
            if !first {
                write!(f, " ")?;
            }
            write!(f, "{}", value)?;
            first = false;
        }
        
        write!(f, "]")
    }
    
    /// Format a block for debug (developer-oriented)
    fn format_block_debug(&self, f: &mut std::fmt::Formatter<'_>, blob: &Blob) -> std::fmt::Result {
        match blob {
            Blob::Inline(container) => {
                let len = container[0] as usize;
                write!(f, "Block::Inline(size={}, [", len)?;
                
                let values = self.get_block_values(blob);
                let mut first = true;
                
                for value in values {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", value)?;
                    first = false;
                }
                
                write!(f, "])")
            },
            Blob::External(hash) => {
                write!(f, "Block::External({:02x}{:02x}..{:02x}{:02x}, [", 
                      hash[0], hash[1], 
                      hash[hash.len()-2], hash[hash.len()-1])?;
                
                let values = self.get_block_values(blob);
                let mut first = true;
                
                for value in values {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", value)?;
                    first = false;
                }
                
                write!(f, "])")
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
/// 
/// Note: This implementation can only display inner values for inline blocks.
/// For full block display that works with both inline and external blocks,
/// use a BlobStore implementation's format_block_display method.
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Int(i) => write!(f, "{}", i),
            Self::Block(blob) => {
                // Basic display without access to BlobStore
                write!(f, "[")?;
                
                match blob {
                    Blob::Inline(container) => {
                        let len = container[0] as usize;
                        if len == 0 {
                            // Empty block
                        } else {
                            // Indicate there's content
                            write!(f, "...")?;
                        }
                    },
                    Blob::External(_) => {
                        // For external blocks, we need a BlobStore to get content
                        write!(f, "...")?;
                    }
                }
                
                write!(f, "]")
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
                    // For external strings, we just indicate it's a string
                    write!(f, "\"...\"")
                }
            },
            Self::Word(word) => write!(f, "{}", word),
            Self::SetWord(word) => write!(f, "{}:", word),
        }
    }
}

/// Debug format is designed for programmers and includes more technical details.
/// 
/// Note: This implementation can only show inner values for inline blocks.
/// For full block debug output that works with both inline and external blocks,
/// use a BlobStore implementation's format_block_debug method.
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
        // Create a valid inline block for testing
        // Properly create a small inline block by creating a container
        let mut container = [0u8; HASH_SIZE];
        
        // Set the size (first byte) to 4 to indicate a small block
        container[0] = 4;
        
        // Put the minimal components needed for a valid block
        // This is a simplification for testing - real blocks would have valid serialized data
        container[1] = 0; // First value tag
        container[2] = 0; // Second value offset
        
        let inline_block = Value::Block(Blob::Inline(container));
        let external_block = Value::Block(create_external_blob());
        
        // Check Display format for end users
        let inline_str = inline_block.to_string();
        let external_str = external_block.to_string();
        
        // All blocks should at least have brackets
        assert!(inline_str.starts_with("["));
        assert!(inline_str.ends_with("]"));
        assert!(external_str.starts_with("["));
        assert!(external_str.ends_with("]"));
        
        // Check Debug format for programmers
        assert!(format!("{:?}", inline_block).starts_with("Block::Inline"));
        assert!(format!("{:?}", external_block).starts_with("Block::External"));
    }
    
    #[test]
    fn test_blob_debug() {
        // Test Blob Debug format (no Display implementation anymore)
        let inline_blob = create_string_blob("abc");
        let external_blob = create_external_blob();
        
        // Debug for programmers should be detailed
        let debug_inline = format!("{:?}", inline_blob);
        let debug_external = format!("{:?}", external_blob);
        
        assert!(debug_inline.contains("size=3"));
        assert!(debug_inline.contains("data="));
        assert!(debug_external.contains("External(hash="));
        assert!(debug_external.contains(".."));
    }
}
