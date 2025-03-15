// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use smol_str::SmolStr;

use crate::hash::hash_u32x8;
use thiserror::Error;

pub type Word = u32;
pub type Offset = Word;
pub type SymbolId = Offset;

// E R R O R

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("unexpected error")]
    UnexpectedError,
    #[error("word not found")]
    WordNotFound,
    #[error("stack underflow")]
    StackUnderflow,
    #[error("stack overflow")]
    StackOverflow,
    #[error("out of memory")]
    OutOfMemory,
    #[error("out of bounds")]
    OutOfBounds,
    #[error("symbol not found")]
    SymbolNotFound,
    #[error("symbol table full")]
    SymbolTableFull,
    #[error("context full")]
    ContextFull,
    #[error("bad symbol")]
    BadSymbol,
    #[error("string too long")]
    StringTooLong,
    #[error(transparent)]
    TryFromSliceError(#[from] std::array::TryFromSliceError),
}

// O P S

#[derive(Debug)]
struct Memory<T>(T);

impl<T> Memory<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self) -> Result<Offset, MemoryError> {
        self.0
            .as_ref()
            .first()
            .copied()
            .ok_or(MemoryError::UnexpectedError)
    }

    fn split_first(&self) -> Option<(&u32, &[Word])> {
        self.0.as_ref().split_first()
    }

    fn get_block(&self, addr: Offset) -> Result<&[Word], MemoryError> {
        self.0
            .as_ref()
            .get(addr as usize + 1..)
            .and_then(|data| {
                data.split_first()
                    .and_then(|(len, block)| block.get(..*len as usize))
            })
            .ok_or(MemoryError::OutOfBounds)
    }

    fn get<const N: usize>(&self, addr: Offset) -> Result<[u32; N], MemoryError> {
        self.0
            .as_ref()
            .split_first()
            .and_then(|(len, data)| {
                let begin = addr as usize;
                let end = begin + N;
                if end <= *len as usize {
                    data.get(begin..end).and_then(|block| block.try_into().ok())
                } else {
                    None
                }
            })
            .ok_or(MemoryError::OutOfBounds)
    }
}

impl<T> Memory<T>
where
    T: AsMut<[Word]>,
{
    /// Set memory from allocation start to provided values
    fn init(&mut self, value: Word) -> Result<(), MemoryError> {
        self.0
            .as_mut()
            .first_mut()
            .map(|slot| *slot = value)
            .ok_or(MemoryError::UnexpectedError)
    }

    fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, MemoryError> {
        self.0
            .as_mut()
            .split_first_mut()
            .and_then(|(len, data)| {
                let addr = *len as usize;
                data.get_mut(addr..addr + N).map(|block| {
                    block
                        .iter_mut()
                        .zip(words.iter())
                        .for_each(|(slot, value)| {
                            *slot = *value;
                        });
                    *len += N as u32;
                    addr as Offset
                })
            })
            .ok_or(MemoryError::OutOfMemory)
    }

    fn split_first_mut(&mut self) -> Option<(&mut u32, &mut [Word])> {
        self.0.as_mut().split_first_mut()
    }

    // fn split_at_mut<const M: usize>(&mut self) -> Option<(&mut [Word; M], &mut [Word])> {
    //     let (header, rest) = self.0.as_mut().split_at_mut(M);
    //     header.try_into().ok().map(|header| (header, rest))
    // }

    fn reserve(&mut self, size: Offset) -> Option<(Offset, &mut [Word])> {
        self.0.as_mut().split_first_mut().and_then(|(len, data)| {
            let start = *len as usize;
            let end = start + size as usize;
            if end <= data.len() {
                data.get_mut(start..end).map(|block| {
                    *len += size;
                    (start as Offset, block)
                })
            } else {
                None
            }
        })
    }

    fn alloc_empty_block(&mut self, size: Offset) -> Option<(Offset, &mut [Word])> {
        self.reserve(size + 1).and_then(|(addr, block)| {
            block.split_first_mut().map(|(len, data)| {
                *len = size;
                (addr, data)
            })
        })
    }

    fn alloc_block(&mut self, values: &[Word]) -> Result<Offset, MemoryError> {
        self.alloc_empty_block(values.len() as Offset)
            .map(|(addr, block)| {
                block
                    .iter_mut()
                    .zip(values.iter())
                    .for_each(|(slot, value)| {
                        *slot = *value;
                    });
                addr
            })
            .ok_or(MemoryError::OutOfMemory)
    }

    fn get_block_mut(&mut self, addr: Offset) -> Result<&mut [Word], MemoryError> {
        self.0
            .as_mut()
            .get_mut(addr as usize + 1..)
            .and_then(|data| {
                data.split_first_mut()
                    .and_then(|(len, block)| block.get_mut(..*len as usize))
            })
            .ok_or(MemoryError::OutOfBounds)
    }

    fn put<const N: usize>(&mut self, addr: Offset, value: [Word; N]) -> Result<(), MemoryError> {
        self.0
            .as_mut()
            .split_first_mut()
            .and_then(|(len, data)| {
                let addr = addr as usize;
                if addr + N <= *len as usize {
                    data.get_mut(addr..addr + N).map(|block| {
                        block
                            .iter_mut()
                            .zip(value.iter())
                            .for_each(|(slot, value)| {
                                *slot = *value;
                            });
                    })
                } else {
                    None
                }
            })
            .ok_or(MemoryError::OutOfBounds)
    }

    fn get_mut<const N: usize>(&mut self, addr: Offset) -> Result<&mut [u32; N], MemoryError> {
        self.0
            .as_mut()
            .split_first_mut()
            .and_then(|(len, data)| {
                let begin = addr as usize;
                let end = begin + N;
                if end <= *len as usize {
                    data.get_mut(begin..end)
                        .and_then(|block| block.try_into().ok())
                } else {
                    None
                }
            })
            .ok_or(MemoryError::OutOfBounds)
    }
}

// B L O C K

// #[derive(Debug)]
// pub struct Block<T>(Memory<T>);

// impl<T> Block<T> {
//     pub fn new(data: T) -> Self {
//         Self(Memory(data))
//     }
// }

// impl<T> Block<T>
// where
//     T: AsRef<[Word]>,
// {
//     pub fn as_ref(&self) -> &[Word] {
//         self.0 .0.as_ref()
//     }
// }

// S T A C K

pub struct Stack<T>(Memory<T>);

impl<T> Stack<T> {
    pub fn new(data: T) -> Self {
        Self(Memory(data))
    }
}

impl<T> Stack<T>
where
    T: AsRef<[Word]>,
{
    pub fn len(&self) -> Result<Offset, MemoryError> {
        self.0.len()
    }

    pub fn is_empty(&self) -> Result<bool, MemoryError> {
        self.len().map(|len| len == 0)
    }

    pub fn get<const N: usize>(&self, offset: Offset) -> Result<[Word; N], MemoryError> {
        self.0.get(offset)
    }

    // pub fn peek_all(&self, offset: Offset) -> Result<&[Word], CoreError> {
    //     self.0
    //         .split_first()
    //         .and_then(|(len, data)| {
    //             len.checked_sub(offset).and_then(|size| {
    //                 let addr = offset as usize;
    //                 data.get(addr..addr + size as usize)
    //             })
    //         })
    //         .ok_or(CoreError::StackUnderflow)
    // }

    pub fn peek<const N: usize>(&self) -> Option<[Word; N]> {
        self.0.split_first().and_then(|(len, data)| {
            len.checked_sub(N as u32).and_then(|offset| {
                let addr = offset as usize;
                data.get(addr..addr + N)
                    .and_then(|block| block.try_into().ok())
            })
        })
    }

    pub fn peek_all(&self, offset: Offset) -> Option<&[Word]> {
        self.0.split_first().and_then(|(len, data)| {
            len.checked_sub(offset).and_then(|size| {
                let addr = offset as usize;
                data.get(addr..addr + size as usize)
            })
        })
    }
}

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    pub fn set_len(&mut self, len: Word) -> Result<(), MemoryError> {
        self.0.init(len)
    }

    pub fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, MemoryError> {
        self.0.alloc(words)
    }

    pub fn push<const N: usize>(&mut self, words: [u32; N]) -> Result<(), MemoryError> {
        self.0.alloc(words).map(|_| ())
    }

    pub fn peek_mut<const N: usize>(&mut self) -> Option<&mut [Word]> {
        self.0.split_first_mut().and_then(|(len, data)| {
            len.checked_sub(N as u32).and_then(|offset| {
                let addr = offset as usize;
                data.get_mut(addr..addr + N)
            })
        })
    }

    pub fn pop<const N: usize>(&mut self) -> Result<[u32; N], MemoryError> {
        self.0
            .split_first_mut()
            .and_then(|(len, data)| {
                len.checked_sub(N as u32).and_then(|new_len| {
                    let addr = new_len as usize;
                    data.get(addr..addr + N).map(|block| {
                        let mut words = [0; N];
                        block
                            .iter()
                            .zip(words.iter_mut())
                            .for_each(|(slot, value)| {
                                *value = *slot;
                            });
                        *len = new_len;
                        words
                    })
                })
            })
            .ok_or(MemoryError::StackUnderflow)
    }

    pub fn pop_all(&mut self, offset: Offset) -> Option<&[Word]> {
        self.0.split_first_mut().and_then(|(len, data)| {
            len.checked_sub(offset).and_then(|size| {
                let addr = offset as usize;
                data.get(addr..addr + size as usize).inspect(|_| {
                    *len = offset;
                })
            })
        })
    }
}

// S Y M B O L   T A B L E

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol([u32; 8]);

impl Symbol {
    pub fn from(string: &str) -> Result<Self, MemoryError> {
        let bytes = string.as_bytes();
        let len = bytes.len();
        if len < 32 {
            let mut buf = [0; 8];
            buf[0] = len as u32;
            for (i, byte) in bytes.iter().enumerate() {
                let j = i + 1;
                buf[j / 4] |= (*byte as u32) << ((j % 4) * 8);
            }
            Ok(Symbol(buf))
        } else {
            Err(MemoryError::StringTooLong)
        }
    }

    pub fn to_string(&self) -> SmolStr {
        let bytes: [u8; 32] = unsafe { std::mem::transmute(self.0) };
        let len = bytes[0] as usize;
        let str = unsafe { std::str::from_utf8_unchecked(&bytes[1..=len]) };
        str.into()
    }
}

/// Symbol table for storing and retrieving symbols
/// Symbols are [u32; 8] values stored starting at symbol_table.len() / 9 offset
/// First symbol_table.len() / 9 words is lookup table for symbol values
pub struct SymbolTable<T>(Memory<T>);

impl<T> SymbolTable<T> {
    pub fn new(data: T) -> Self {
        Self(Memory(data))
    }
}

impl<T> SymbolTable<T>
where
    T: AsRef<[Word]>,
{
    pub fn get(&self, symbol: SymbolId) -> Result<Symbol, MemoryError> {
        // Symbol IDs start at 1, so return None for symbol 0
        if symbol == 0 {
            return Err(MemoryError::BadSymbol);
        }

        let (_, data) = self.0.split_first().ok_or(MemoryError::UnexpectedError)?;
        let symbols = data.len() / 9;

        // Check if symbol is out of bounds
        if symbol
            > *self
                .0
                 .0
                .as_ref()
                .first()
                .ok_or(MemoryError::UnexpectedError)?
        {
            return Err(MemoryError::BadSymbol);
        }

        let offset = symbols + (symbol - 1) as usize * 8;

        // Ensure offset is within bounds
        if offset + 8 > data.len() {
            return Err(MemoryError::BadSymbol);
        }

        let bytes = data
            .get(offset..offset + 8)
            .ok_or(MemoryError::OutOfBounds)?;
        Ok(Symbol(bytes.try_into()?))
    }
}

impl<T> SymbolTable<T>
where
    T: AsMut<[Word]>,
{
    pub fn init(&mut self) -> Result<(), MemoryError> {
        self.0.init(0)
    }

    pub fn get_or_insert(&mut self, sym: Symbol) -> Result<SymbolId, MemoryError> {
        let (count, data) = self
            .0
            .split_first_mut()
            .ok_or(MemoryError::UnexpectedError)?;

        let symbols = data.len() / 9;
        if symbols == 0 {
            return Err(MemoryError::SymbolTableFull);
        }

        let h = hash_u32x8(sym.0) as usize;
        let mut index = h % symbols;
        let mut probe = 0;

        loop {
            let pos = data.get(index).copied().ok_or(MemoryError::OutOfBounds)?;
            if pos == 0 {
                *count += 1;
                let pos = *count;

                data.get_mut(index)
                    .map(|slot| *slot = pos)
                    .ok_or(MemoryError::UnexpectedError)?;

                let offset = symbols + (pos - 1) as usize * 8;
                let value = data
                    .get_mut(offset..offset + 8)
                    .ok_or(MemoryError::OutOfBounds)?;
                value.iter_mut().zip(sym.0.iter()).for_each(|(dst, src)| {
                    *dst = *src;
                });
                return Ok(pos);
            } else {
                let offset = symbols + (pos - 1) as usize * 8;
                let value = data
                    .get_mut(offset..offset + 8)
                    .ok_or(MemoryError::OutOfBounds)?;
                if value == sym.0 {
                    return Ok(pos);
                }
            }
            index = (index + 1) % symbols;
            probe += 1;
            if probe >= symbols {
                return Err(MemoryError::SymbolTableFull);
            }
        }
    }
}

// C O N T E X T

/// A Context represents a dictionary-like structure mapping symbols to values.
/// It is implemented as a hash table that maps Symbol IDs to [Word; 2] pairs.
#[derive(Debug)]
pub struct Context<T>(Memory<T>);

/// An iterator over the entries in a Context, yielding (Symbol, [Word; 2]) pairs.
///
/// This provides efficient iteration over all entries in a Context without having
/// to manually check for empty slots or handle hash table collisions. It transparently
/// skips empty slots and handles the underlying hash table structure.
///
/// # Examples
///
/// ```
/// use rebel::mem::{Context, Word};
///
/// // Initialize a context
/// let mut buffer = vec![0u32; 100];
/// let mut context = Context::new(buffer.as_mut_slice());
/// context.init().unwrap();
///
/// // Add some key-value pairs
/// context.put(1, [10, 20]).unwrap();
/// context.put(2, [30, 40]).unwrap();
///
/// // Iterate using iter() method
/// for (symbol, value) in context.iter() {
///     println!("Symbol: {}, Value: {:?}", symbol, value);
/// }
///
/// // Or use into_iter directly for a more concise syntax
/// for (symbol, value) in &context {
///     // Process each key-value pair
///     assert!(symbol > 0, "Symbol IDs start at 1");
///     assert_eq!(value.len(), 2, "Values are [Word; 2] arrays");
/// }
/// ```
pub struct ContextIterator<'a, T: 'a> {
    context: &'a Context<T>,
    capacity: usize,
    current_index: usize,
    count_found: usize,
    total_entries: usize,
}

impl<T> Iterator for ContextIterator<'_, T>
where
    T: AsRef<[Word]>,
{
    type Item = (SymbolId, [Word; 2]);

    fn next(&mut self) -> Option<Self::Item> {
        // Early return if we've found all entries or exhausted the capacity
        if self.count_found >= self.total_entries || self.current_index >= self.capacity {
            return None;
        }

        // Skip empty slots and find the next valid entry
        while self.current_index < self.capacity {
            let index = self.current_index;
            self.current_index += 1; // Advance the index for next iteration

            if let Some(entry) = self.context.get_entry_at(index) {
                self.count_found += 1; // Increment count of found entries
                return Some(entry);
            }
        }

        None // No more entries found
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.total_entries.saturating_sub(self.count_found);
        (remaining, Some(remaining))
    }
}

// Implement IntoIterator for &Context
impl<'a, T> IntoIterator for &'a Context<T>
where
    T: AsRef<[Word]>,
{
    type Item = (SymbolId, [Word; 2]);
    type IntoIter = ContextIterator<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> Context<T> {
    const ENTRY_SIZE: usize = 3;

    pub fn new(data: T) -> Self {
        Self(Memory(data))
    }

    fn hash_u32(val: u32) -> u32 {
        const GOLDEN_RATIO: u32 = 0x9E3779B9;
        val.wrapping_mul(GOLDEN_RATIO)
    }
}

impl<T> Context<T>
where
    T: AsRef<[Word]>,
{
    /// Retrieves the number of entries in the context.
    ///
    /// This reads the entry count from the context header, which is updated
    /// whenever entries are added to the context.
    ///
    /// # Returns
    /// * `Some(count)` - The number of entries in the context
    /// * `None` - If the context header couldn't be read
    pub fn entry_count(&self) -> Option<usize> {
        self.0.split_first().map(|(count, _)| *count as usize)
    }

    /// Returns an iterator over all entries in the context.
    ///
    /// This provides an efficient way to iterate over all key-value pairs in the context
    /// without having to know the symbol IDs in advance or handle the hash table structure.
    /// It returns a [`ContextIterator`] that yields `(Symbol, [Word; 2])` pairs for each entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use rebel::mem::{Context, Word};
    ///
    /// // Create and populate a context
    /// let mut buffer = vec![0u32; 100];
    /// let mut context = Context::new(buffer.as_mut_slice());
    /// context.init().unwrap();
    /// context.put(1, [10, 20]).unwrap();
    /// context.put(2, [30, 40]).unwrap();
    ///
    /// // Collect all entries into a Vec
    /// let entries: Vec<_> = context.iter().collect();
    /// assert_eq!(entries.len(), 2);
    ///
    /// // Find an entry with a specific symbol ID
    /// let entry = context.iter().find(|(symbol, _)| *symbol == 1);
    /// assert!(entry.is_some());
    /// ```
    ///
    /// See [`ContextIterator`] for more examples and details.
    pub fn iter(&self) -> ContextIterator<'_, T> {
        let capacity = self
            .0
            .split_first()
            .map(|(_, data)| data.len() / Self::ENTRY_SIZE)
            .unwrap_or(0);

        let total_entries = self.entry_count().unwrap_or(0);

        ContextIterator {
            context: self,
            capacity,
            current_index: 0,
            count_found: 0,
            total_entries,
        }
    }

    /// Get a value for a given symbol
    pub fn get(&self, symbol: SymbolId) -> Result<[Word; 2], MemoryError> {
        let (_, data) = self.0.split_first().ok_or(MemoryError::UnexpectedError)?;

        let capacity = data.len() / Self::ENTRY_SIZE;
        if capacity == 0 {
            return Err(MemoryError::WordNotFound);
        }

        let h = Self::hash_u32(symbol) as usize;
        let mut index = h % capacity;

        for _probe in 0..capacity {
            let offset = index * Self::ENTRY_SIZE;
            if let Some(found) = data
                .get(offset..offset + Self::ENTRY_SIZE)
                .and_then(|entry| {
                    entry.split_first().and_then(|(cur, val)| {
                        if *cur == symbol {
                            Some([val[0], val[1]])
                        } else {
                            None
                        }
                    })
                })
            {
                return Ok(found);
            }
            index = (index + 1) % capacity;
        }

        Err(MemoryError::WordNotFound)
    }

    /// Retrieves an entry at the given index if it contains a valid symbol.
    ///
    /// This is an internal method used by the ContextIterator to efficiently
    /// iterate through the context's hash table.
    ///
    /// # Parameters
    /// * `index` - The index in the hash table to check
    ///
    /// # Returns
    /// * `Some((symbol, value))` - If a valid entry was found at the index
    /// * `None` - If the index is out of bounds or the slot is empty (symbol = 0)
    ///
    /// # Implementation Notes
    /// The context is implemented as a hash table with linear probing for collision
    /// resolution. Each entry consists of 3 words:
    /// 1. The symbol ID (0 if the slot is empty)
    /// 2. The first word of the value
    /// 3. The second word of the value
    fn get_entry_at(&self, index: usize) -> Option<(SymbolId, [Word; 2])> {
        let (_, data) = self.0.split_first()?;

        let capacity = data.len() / Self::ENTRY_SIZE;
        if index >= capacity {
            return None;
        }

        let offset = index * Self::ENTRY_SIZE;
        data.get(offset..offset + Self::ENTRY_SIZE)
            .and_then(|entry| {
                entry.split_first().and_then(|(symbol, values)| {
                    // Check if this is a valid entry (symbol != 0)
                    if *symbol != 0 {
                        // Extract the value pair
                        let value = [values[0], values[1]];
                        Some((*symbol, value))
                    } else {
                        None
                    }
                })
            })
    }
}

impl<T> Context<T>
where
    T: AsMut<[Word]>,
{
    /// Initialize an empty context.
    ///
    /// This sets up an empty context ready for use.
    pub fn init(&mut self) -> Result<(), MemoryError> {
        self.0.init(0)
    }

    /// Get a value for a given symbol
    // pub fn replace(&mut self, symbol: SymbolId, value: [Word; 2]) -> Result<(), MemoryError> {
    //     let (_, data) = self
    //         .0
    //         .split_first_mut()
    //         .ok_or(MemoryError::UnexpectedError)?;

    //     let capacity = data.len() / Self::ENTRY_SIZE;
    //     if capacity == 0 {
    //         return Err(MemoryError::WordNotFound);
    //     }

    //     let h = Self::hash_u32(symbol) as usize;
    //     let mut index = h % capacity;

    //     for _probe in 0..capacity {
    //         let offset = index * Self::ENTRY_SIZE;
    //         if let Some(found) =
    //             data.get_mut(offset..offset + Self::ENTRY_SIZE)
    //                 .and_then(|entry| {
    //                     entry.split_first_mut().and_then(|(cur, val)| {
    //                         if *cur == symbol {
    //                             Some(val)
    //                         } else {
    //                             None
    //                         }
    //                     })
    //                 })
    //         {
    //             found[0] = value[0];
    //             found[1] = value[1];
    //             return Ok(());
    //         }
    //         index = (index + 1) % capacity;
    //     }

    //     Err(MemoryError::WordNotFound)
    // }

    pub fn seal(&mut self) -> Result<(), MemoryError> {
        let (header, _) = self
            .0
            .split_first_mut()
            .ok_or(MemoryError::UnexpectedError)?;
        *header |= 0x8000_0000;
        Ok(())
    }

    pub fn put(&mut self, symbol: SymbolId, value: [Word; 2]) -> Result<(), MemoryError> {
        let (header, data) = self
            .0
            .split_first_mut()
            .ok_or(MemoryError::UnexpectedError)?;

        let sealed = *header & 0x8000_0000;
        let count = *header & 0x7FFF_FFFF;

        let capacity = data.len() / Self::ENTRY_SIZE;
        if capacity == 0 {
            return Err(MemoryError::SymbolTableFull);
        }

        let h = Self::hash_u32(symbol) as usize;
        let mut index = h % capacity;

        for _probe in 0..capacity {
            let offset = index * Self::ENTRY_SIZE;
            let entry = data
                .get_mut(offset..offset + Self::ENTRY_SIZE)
                .ok_or(MemoryError::OutOfBounds)?;
            let (cur, val) = entry
                .split_first_mut()
                .ok_or(MemoryError::UnexpectedError)?;
            if *cur == 0 {
                if sealed != 0 {
                    return Err(MemoryError::WordNotFound);
                }
                *header = count + 1;
                *cur = symbol;
            }
            if *cur == symbol {
                val[0] = value[0];
                val[1] = value[1];
                return Ok(());
            }
            index = (index + 1) % capacity;
        }
        if sealed != 0 {
            Err(MemoryError::WordNotFound)
        } else {
            Err(MemoryError::ContextFull)
        }
    }
}

// H E A P

pub struct Heap<T>(Memory<T>);

impl<T> Heap<T> {
    pub fn new(data: T) -> Self {
        Self(Memory(data))
    }
}

impl<T> Heap<T>
where
    T: AsRef<[Word]>,
{
    pub fn get_block(&self, addr: Offset) -> Result<&[Word], MemoryError> {
        self.0.get_block(addr)
    }

    pub fn get<const N: usize>(&self, addr: Offset) -> Result<[Word; N], MemoryError> {
        self.0.get(addr)
    }
}

impl<T> Heap<T>
where
    T: AsMut<[Word]>,
{
    pub fn init(&mut self, reserve: u32) -> Result<(), MemoryError> {
        self.0.init(reserve)
    }

    pub fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, MemoryError> {
        self.0.alloc(words)
    }

    pub fn alloc_empty_block(
        &mut self,
        size: Offset,
    ) -> Result<(Offset, &mut [Word]), MemoryError> {
        self.0
            .alloc_empty_block(size)
            .ok_or(MemoryError::OutOfMemory)
    }

    pub fn alloc_block(&mut self, values: &[Word]) -> Result<Offset, MemoryError> {
        self.0.alloc_block(values)
    }

    pub fn get_block_mut(&mut self, addr: Offset) -> Result<&mut [Word], MemoryError> {
        self.0.get_block_mut(addr)
    }

    pub fn put<const N: usize>(
        &mut self,
        addr: Offset,
        value: [Word; N],
    ) -> Result<(), MemoryError> {
        self.0.put(addr, value)
    }

    pub fn get_mut<const N: usize>(&mut self, addr: Offset) -> Result<&mut [u32; N], MemoryError> {
        self.0.get_mut(addr)
    }

    //

    pub fn alloc_context(&mut self, size: Offset) -> Result<Offset, MemoryError> {
        let (addr, data) =
            self.alloc_empty_block(size * (Context::<T>::ENTRY_SIZE as Offset) + 1)?;
        Context::new(data).init()?;
        Ok(addr)
    }
}

//

// pub fn inline(string: &str) -> Result<[u32; 8], CoreError> {
//     inline_string(string)
// }

// pub fn alloc(stack: &mut Stack<&mut [Word]>, a: u32, b: u32, c: u32) -> Option<Offset> {
//     stack.alloc([a, b, c])
// }

// pub fn push_r(stack: &mut Stack<&mut [Word]>, a: u32, b: u32, c: u32) -> Option<()> {
//     stack.push([a, b, c])
// }

// pub fn push_a(stack: &mut Stack<[Word; 128]>, a: u32, b: u32, c: u32) -> Option<()> {
//     stack.push([a, b, c])
// }

// pub fn get_block_mut<'a>(
//     block: &'a mut Block<&mut [Word]>,
//     addr: Offset,
// ) -> Option<&'a mut [Word]> {
//     block.get_block_mut(addr)
// }

// pub fn pop(stack: &mut Stack<&mut [Word]>) -> Option<[u32; 3]> {
//     stack.pop()
// }

// pub fn context_get<'a>(
//     ctx: &'a mut Context<&mut [Word]>,
//     symbol: Offset,
// ) -> Result<[Word; 2], CoreError> {
//     ctx.get(symbol)
// }

// pub fn context_put<'a>(
//     ctx: &'a mut Context<&mut [Word]>,
//     symbol: Offset,
//     value: [Word; 2],
// ) -> Result<(), CoreError> {
//     ctx.put(symbol, value)
// }

// pub fn symbol_get(
//     table: &mut SymbolTable<&mut [Word]>,
//     sym: [u32; 8],
// ) -> Result<Symbol, CoreError> {
//     table.get_or_insert(sym)
// }

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_context_iterator() {
        // Create a buffer for the context
        let mut buffer = vec![0u32; 100];

        // Initialize context with some entries
        let mut context = Context::new(buffer.as_mut_slice());
        context.init().expect("Failed to initialize context");

        // Insert some key-value pairs
        let symbol1 = 1;
        let value1 = [10, 20];
        context
            .put(symbol1, value1)
            .expect("Failed to insert first entry");

        let symbol2 = 2;
        let value2 = [30, 40];
        context
            .put(symbol2, value2)
            .expect("Failed to insert second entry");

        let symbol3 = 3;
        let value3 = [50, 60];
        context
            .put(symbol3, value3)
            .expect("Failed to insert third entry");

        // Create a hashmap to track what we've found
        let mut expected = HashMap::new();
        expected.insert(symbol1, value1);
        expected.insert(symbol2, value2);
        expected.insert(symbol3, value3);

        // Use the iterator to collect all entries
        let entries: Vec<_> = context.iter().collect();

        // Verify we found the correct number of entries
        assert_eq!(entries.len(), 3, "Iterator should return exactly 3 entries");

        // Verify all entries match what we inserted
        for (symbol, value) in entries {
            assert!(
                expected.contains_key(&symbol),
                "Found unexpected symbol: {}",
                symbol
            );
            assert_eq!(
                expected[&symbol], value,
                "Value for symbol {} doesn't match expected",
                symbol
            );
        }

        // Test the IntoIterator implementation with a for loop
        let mut found_count = 0;
        for (symbol, value) in &context {
            found_count += 1;
            assert!(
                expected.contains_key(&symbol),
                "Found unexpected symbol: {}",
                symbol
            );
            assert_eq!(
                expected[&symbol], value,
                "Value for symbol {} doesn't match expected",
                symbol
            );
        }

        assert_eq!(found_count, 3, "For loop should visit exactly 3 entries");
    }

    #[test]
    fn test_symbol_table_init_get_insert() {
        // Create a buffer large enough for the symbol table (9 slots per symbol)
        let mut buffer = vec![0u32; 100];
        let mut table = SymbolTable::new(buffer.as_mut_slice());

        // Initialize the table
        table.init().expect("Failed to initialize symbol table");

        // Insert a new symbol
        let sym1 = Symbol([1u32, 2, 3, 4, 5, 6, 7, 8]);
        let symbol1 = table.get_or_insert(sym1).expect("Failed to insert symbol");

        // Verify we can retrieve it
        let retrieved = table.get(symbol1).expect("Failed to get inserted symbol");
        assert_eq!(
            retrieved, sym1,
            "Retrieved symbol doesn't match inserted symbol"
        );

        // Insert another symbol
        let sym2 = Symbol([10u32, 20, 30, 40, 50, 60, 70, 80]);
        let symbol2 = table
            .get_or_insert(sym2)
            .expect("Failed to insert second symbol");

        // Verify we can retrieve both symbols correctly
        let retrieved1 = table.get(symbol1).expect("Failed to get first symbol");
        let retrieved2 = table.get(symbol2).expect("Failed to get second symbol");
        assert_eq!(retrieved1, sym1, "First retrieved symbol doesn't match");
        assert_eq!(retrieved2, sym2, "Second retrieved symbol doesn't match");
        assert_ne!(
            symbol1, symbol2,
            "Different symbols should get different IDs"
        );

        // Verify that inserting an existing symbol returns the same ID
        let symbol1_again = table
            .get_or_insert(sym1)
            .expect("Failed to re-insert first symbol");
        assert_eq!(
            symbol1, symbol1_again,
            "Re-inserting same symbol should return same ID"
        );
    }

    #[test]
    fn test_symbol_table_collision_handling() {
        // Create a buffer with limited size to force collisions
        // Use a small prime number (11) for the hash table size for better testing of collision handling
        let mut buffer = vec![0u32; 11 * 9 + 1]; // 11 slots + overhead for symbol storage
        let mut table = SymbolTable::new(buffer.as_mut_slice());

        table.init().expect("Failed to initialize symbol table");

        // Insert multiple symbols to test collision handling
        let mut symbols = Vec::new();
        let mut symbol_ids = Vec::new();

        // Create and insert 8 different symbols
        for i in 0..8 {
            let sym = Symbol([i + 1, i + 2, i + 3, i + 4, i + 5, i + 6, i + 7, i + 8]);
            symbols.push(sym);
            let id = table.get_or_insert(sym).expect("Failed to insert symbol");
            symbol_ids.push(id);
        }

        // Verify all symbols can be retrieved correctly
        for (i, &id) in symbol_ids.iter().enumerate() {
            let retrieved = table.get(id).expect("Failed to get symbol");
            assert_eq!(
                retrieved, symbols[i],
                "Symbol {} not retrieved correctly",
                i
            );
        }

        // Verify that reinserting returns the same ID
        for (i, &sym) in symbols.iter().enumerate() {
            let id = table.get_or_insert(sym).expect("Failed to reinsert symbol");
            assert_eq!(
                id, symbol_ids[i],
                "Reinserting symbol returned different ID"
            );
        }
    }

    #[test]
    fn test_symbol_table_error_conditions() {
        // Test with invalid symbol ID
        let mut buffer = vec![0u32; 100];
        let mut table = SymbolTable::new(buffer.as_mut_slice());

        // Initialize the table
        table.init().expect("Failed to initialize symbol table");

        // Try to get a symbol that doesn't exist
        assert!(
            table.get(999).is_err(),
            "Getting non-existent symbol should return None"
        );

        // Symbol index 0 should be invalid (symbols start at 1)
        assert!(
            table.get(0).is_err(),
            "Getting symbol with ID 0 should return None"
        );

        // Test empty buffer scenario - a buffer that's technically big enough but too small to be practical
        // A proper symbol table needs at least 1 slot in the hash table + space for the symbol
        let mut small_buffer = vec![0u32; 1]; // Header only, no space for symbols
        let mut small_table = SymbolTable::new(small_buffer.as_mut_slice());
        small_table
            .init()
            .expect("Failed to initialize small table");

        let sym = Symbol([1u32, 2, 3, 4, 5, 6, 7, 8]);
        assert!(
            small_table.get_or_insert(sym).is_err(),
            "Inserting into table without enough space should return None"
        );
    }
}
