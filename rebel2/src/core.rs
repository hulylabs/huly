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

struct ArrayIter<'a, T, I> {
    array: &'a Array<T, I>,
    index: usize,
}

impl<'a, T, I> Iterator for ArrayIter<'a, T, I>
where
    T: AsRef<[u8]>,
{
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.array.get(self.index);
        self.index += 1;
        value
    }
}

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

    fn len(&self) -> Option<usize> {
        Self::get_index(self.0.as_ref(), 0)
    }

    fn get(&self, index: usize) -> Option<Value> {
        let data = self.0.as_ref();
        let items = Self::get_index(data, 0)?;

        if index < items {
            let offset_pos = data.len() - (index + 1) * Self::INDEX_SIZE;
            let offset = Self::get_index(data, offset_pos)?;
            data.get(offset..).and_then(Value::load)
        } else {
            None
        }
    }

    fn iter(&self) -> ArrayIter<'_, T, I> {
        ArrayIter {
            array: self,
            index: 0,
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

const HASH_SIZE: usize = 32;
pub type Hash = [u8; HASH_SIZE];

pub const INLINE_MAX: usize = 34;

#[derive(Clone, Debug)]
struct Inline {
    size: u8,
    data: [u8; INLINE_MAX],
}

impl Inline {
    fn new(len: usize, data: &[u8]) -> Option<Self> {
        if len > INLINE_MAX {
            None
        } else {
            let mut inline = [0; INLINE_MAX];
            data.get(..len)
                .map(|src| inline[..len].copy_from_slice(src))?;

            Some(Self {
                size: len as u8,
                data: inline,
            })
        }
    }

    fn load(data: &[u8]) -> Option<Self> {
        data.split_first().and_then(|(len, data)| {
            let len = *len as usize;
            Inline::new(len, data)
        })
    }

    fn as_slice(&self) -> &[u8] {
        &self.data[..self.size as usize]
    }

    fn as_array(&self) -> Array<&[u8], u8> {
        Array::new(self.as_slice())
    }
}

#[derive(Clone, Debug)]
pub enum Blob {
    Inline(Inline),
    External(Hash),
}

impl Blob {
    const EXTERNAL: u8 = 0x10;

    fn load(options: u8, data: &[u8]) -> Option<Blob> {
        if options == Self::EXTERNAL {
            data.get(..32)?.try_into().ok().map(Blob::External)
        } else {
            Inline::load(data).map(Blob::Inline)
        }
    }

    fn write<W: std::io::Write>(&self, write: &mut W, tag: u8) -> Result<(), CoreError> {
        let _ = match self {
            Blob::Inline(inline) => write.write_vectored(&[
                IoSlice::new(&[tag, inline.size]),
                IoSlice::new(inline.as_slice()),
            ])?,
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

    fn create_blob(&mut self, data: &[u8]) -> Result<Blob, CoreError> {
        let len = data.len();
        if len <= INLINE_MAX {
            let inline = Inline::new(len, data).ok_or(CoreError::InternalError)?;
            Ok(Blob::Inline(inline))
        } else {
            self.put(data).map(Blob::External)
        }
    }

    fn create_block<T: BlobStore>(
        &mut self,
        data: &[u8],
        offsets: &[usize],
    ) -> Result<Block, CoreError> {
        let min_size = 1 + data.len() + offsets.len();
        if min_size <= INLINE_MAX {
            let mut container = [0; INLINE_MAX];
            container[0] = offsets.len() as u8;
            container[1..data.len() + 1].copy_from_slice(data);
            container
                .iter_mut()
                .skip(data.len() + 1)
                .zip(offsets.iter().rev())
                .for_each(|(i, offset)| {
                    *i = *offset as u8;
                });
            Ok(Block(Blob::Inline(
                Inline::new(min_size, &container).ok_or(CoreError::InternalError)?,
            )))
        } else {
            let size = 4 * 1 + data.len() + 4 * offsets.len();
            let mut blob_data = Vec::with_capacity(size);
            blob_data.extend_from_slice(&u32::to_le_bytes(offsets.len() as u32));
            blob_data.extend_from_slice(data);
            for offset in offsets.iter().rev() {
                blob_data.extend_from_slice(&u32::to_le_bytes(*offset as u32));
            }
            let hash = self.put(&blob_data)?;
            Ok(Block(Blob::External(hash)))
        }
    }
}

//

#[derive(Debug, Clone)]
pub struct Block(Blob);

impl Block {
    pub fn get<T: BlobStore>(&self, store: &T, index: usize) -> Option<Value> {
        match &self.0 {
            Blob::Inline(inline) => Array::<&[u8], u8>::new(inline.as_slice()).get(index),
            Blob::External(hash) => {
                let data = store.get(hash).ok()?;
                Array::<&[u8], u32>::new(data).get(index)
            }
        }
    }
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Blob::Inline(inline) => {
                write!(f, "[")?;
                let array = inline.as_array();
                for (i, value) in array.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
            Blob::External(_) => write!(f, "[...]"),
        }
    }
}

//

#[derive(Debug, Clone)]
pub enum Value {
    None,
    Int(i64),
    // Float(f64),
    Block(Block),
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
            Self::Block(block) => write!(f, "{}", block),
            Self::String(blob) => match blob {
                Blob::Inline(inline) => {
                    if let Ok(s) = std::str::from_utf8(inline.as_slice()) {
                        write!(f, "\"{}\"", s)
                    } else {
                        write!(f, "\"<binary data>\"")
                    }
                }
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
            Self::TAG_BLOCK => Blob::load(header & Self::OPTION_MASK, &data[1..])
                .map(Block)
                .map(Self::Block),
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
            Self::Block(block) => block.0.write(write, Self::TAG_BLOCK),
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
        let inline = Inline::new(len, bytes).unwrap();
        Blob::Inline(inline)
    }

    /// Helper to create an external blob with a mock hash
    fn create_external_blob() -> Blob {
        let mut hash = [0u8; HASH_SIZE];
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
    }

    // #[test]
    // fn test_display_block_values() {
    //     let inline_block = Value::Block(Blob::Inline(Inline::new(0, &[]).unwrap()));
    //     let external_block = Value::Block(create_external_blob());

    //     // Check Display format for end users
    //     let inline_str = inline_block.to_string();
    //     let external_str = external_block.to_string();

    //     // All blocks should at least have brackets
    //     assert!(inline_str.starts_with("["));
    //     assert!(inline_str.ends_with("]"));
    //     assert!(external_str.starts_with("["));
    //     assert!(external_str.ends_with("]"));
    // }
}
