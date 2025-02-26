// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use smol_str::SmolStr;
use std::array::TryFromSliceError;
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
    fn write(self, dst: &mut [u8], tag: u8, symbol: &SmolStr) -> Option<usize> {
        if symbol.len() > 0xff {
            None
        } else {
            dst.get_mut(0..2).map(|bytes| {
                bytes[0] = tag | (self as u8) << 3;
                bytes[1] = symbol.len() as u8;
            })?;
            dst.get_mut(2..2 + symbol.len())
                .map(|bytes| bytes.copy_from_slice(symbol.as_bytes()))?;
            Some(2 + symbol.len())
        }
    }

    fn load(data: &[u8]) -> Option<SmolStr> {
        data.split_first().and_then(|(len, data)| {
            let len = *len as usize;
            let bytes = data.get(..len)?;
            let str = unsafe { std::str::from_utf8_unchecked(bytes) };
            Some(SmolStr::from(str))
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

    fn write(&self, dst: &mut [u8], tag: u8) -> Option<usize> {
        match self {
            Blob::Inline(inline) => {
                let size = inline.size as usize;
                dst.get_mut(0..2).map(|bytes| {
                    bytes[0] = tag;
                    bytes[1] = inline.size;
                })?;
                dst.get_mut(2..2 + size)
                    .map(|bytes| bytes.copy_from_slice(inline.as_slice()))?;
                Some(2 + size)
            }
            Blob::External(hash) => {
                dst.get_mut(0..1)
                    .map(|bytes| bytes[0] = tag | Self::EXTERNAL)?;
                dst.get_mut(1..33)
                    .map(|bytes| bytes.copy_from_slice(hash))?;
                Some(33)
            }
        }
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

    pub fn new_inline(values: &[Value]) -> Option<Self> {
        let mut container = [0; INLINE_MAX];
        container[0] = values.len() as u8;

        let mut data_ptr = 1;
        let mut offset_ptr = INLINE_MAX;

        for item in values.iter() {
            offset_ptr -= 1;
            container
                .get_mut(offset_ptr)
                .map(|offset_ptr| *offset_ptr = data_ptr as u8)?;
            data_ptr += container
                .get_mut(data_ptr..)
                .and_then(|data| item.write(data))?;
            if data_ptr > offset_ptr {
                return None;
            }
        }

        for i in 0..values.len() {
            if data_ptr + i >= INLINE_MAX || offset_ptr + i >= INLINE_MAX {
                return None;
            }
            container[data_ptr + i] = container[offset_ptr + i];
        }

        Inline::new(data_ptr + values.len(), &container)
            .map(Blob::Inline)
            .map(Block)
    }

    pub fn new<T: BlobStore>(store: &mut T, values: &[Value]) -> Result<Self, CoreError> {
        match Self::new_inline(values) {
            Some(block) => Ok(block),
            None => {
                let mut buf = Box::new([0u8; 65536]);
                let mut blob = Vec::new();
                let mut offsets = Vec::new();
                blob.extend_from_slice(&u32::to_le_bytes(values.len() as u32));
                for value in values.iter() {
                    offsets.push(blob.len());
                    let len = value.write(&mut buf[..]).ok_or(CoreError::OutOfBounds)?;
                    blob.extend_from_slice(&buf[..len]);
                }
                for offset in offsets.iter().rev() {
                    blob.extend_from_slice(&u32::to_le_bytes(*offset as u32));
                }
                Ok(Block(Blob::External(store.put(&blob)?)))
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

    pub fn write(&self, dst: &mut [u8]) -> Option<usize> {
        match self {
            Self::None => dst.get_mut(0).map(|byte| {
                *byte = Self::TAG_NONE;
                1
            }),
            Self::Int(value) => dst.get_mut(0..9).map(|bytes| {
                bytes[0] = Self::TAG_INT;
                for i in 0..8 {
                    bytes[8 - i] = (value >> (i * 8)) as u8;
                }
                9
            }),
            Self::Block(block) => block.0.write(dst, Self::TAG_BLOCK),
            Self::String(blob) => blob.write(dst, Self::TAG_STRING),
            Self::Word(word) => WordKind::Word.write(dst, Self::TAG_WORD, word),
            Self::SetWord(word) => WordKind::SetWord.write(dst, Self::TAG_WORD, word),
        }
    }

    pub fn new_block<T: BlobStore>(store: &mut T, values: &[Value]) -> Result<Self, CoreError> {
        Block::new(store, values).map(Self::Block)
    }
}

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

//

pub fn load_value(data: &[u8]) -> Option<Value> {
    Value::load(data)
}

pub fn create_block(values: &[Value]) -> Option<Value> {
    Block::new_inline(values).map(Value::Block)
}

//

#[cfg(test)]
mod tests {
    use super::*;

    fn create_string_blob(s: &str) -> Blob {
        let bytes = s.as_bytes();
        let len = bytes.len();
        let inline = Inline::new(len, bytes).unwrap();
        Blob::Inline(inline)
    }

    fn create_external_blob() -> Blob {
        let mut hash = [0u8; HASH_SIZE];
        for i in 0..HASH_SIZE {
            hash[i] = (i as u8) % 255;
        }
        Blob::External(hash)
    }

    #[test]
    fn test_display_basic_values() {
        assert_eq!(Value::None.to_string(), "none");
        assert_eq!(Value::Int(42).to_string(), "42");
        assert_eq!(Value::Word(SmolStr::new("hello")).to_string(), "hello");
        assert_eq!(Value::SetWord(SmolStr::new("x")).to_string(), "x:");
    }

    #[test]
    fn test_display_string_values() {
        let empty_string = Value::String(create_string_blob(""));
        let short_string = Value::String(create_string_blob("hello"));
        let external_string = Value::String(create_external_blob());

        assert_eq!(empty_string.to_string(), "\"\"");
        assert_eq!(short_string.to_string(), "\"hello\"");
        assert_eq!(external_string.to_string(), "\"...\"");
    }

    #[test]
    fn test_display_block_values() {
        let block_items = vec![Value::Int(1), Value::Int(2), Value::Int(3)];

        let block = Block::new_inline(&block_items).unwrap();
        println!("block: {}", block);
    }
}
