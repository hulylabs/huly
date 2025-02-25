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

#[derive(Debug)]
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

#[derive(Debug, Clone)]
pub enum Blob {
    Inline(Hash),
    External(Hash),
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

#[derive(Debug, Clone)]
pub enum Value {
    None,
    Int(i64),
    // Float(f64),
    Block(Blob),
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
