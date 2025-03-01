// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::CoreError;
use crate::hash::hash_u32x8;

pub type Word = u32;
pub type Offset = Word;
pub type Symbol = Offset;

// O P S

#[derive(Debug)]
struct Memory<T>(T);

impl<T> Memory<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self) -> Option<Offset> {
        self.0.as_ref().first().copied()
    }

    fn split_first(&self) -> Option<(&u32, &[Word])> {
        self.0.as_ref().split_first()
    }

    fn get_block(&self, addr: Offset) -> Option<&[Word]> {
        self.0.as_ref().get(addr as usize + 1..).and_then(|data| {
            data.split_first()
                .and_then(|(len, block)| block.get(..*len as usize))
        })
    }

    fn get<const N: usize>(&self, addr: Offset) -> Option<[u32; N]> {
        self.0.as_ref().split_first().and_then(|(len, data)| {
            let begin = addr as usize;
            let end = begin + N;
            if end <= *len as usize {
                data.get(begin..end).and_then(|block| block.try_into().ok())
            } else {
                None
            }
        })
    }
}

impl<T> Memory<T>
where
    T: AsMut<[Word]>,
{
    /// Set memory from allocation start to provided values
    fn init(&mut self, value: Word) -> Option<()> {
        self.0.as_mut().first_mut().map(|slot| *slot = value)
    }

    fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Option<Offset> {
        self.0.as_mut().split_first_mut().and_then(|(len, data)| {
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

    fn alloc_block(&mut self, values: &[Word]) -> Option<Offset> {
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
    }

    fn get_block_mut(&mut self, addr: Offset) -> Option<&mut [Word]> {
        self.0
            .as_mut()
            .get_mut(addr as usize + 1..)
            .and_then(|data| {
                data.split_first_mut()
                    .and_then(|(len, block)| block.get_mut(..*len as usize))
            })
    }

    fn put<const N: usize>(&mut self, addr: Offset, value: [Word; N]) -> Option<()> {
        self.0.as_mut().split_first_mut().and_then(|(len, data)| {
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
    }

    fn get_mut<const N: usize>(&mut self, addr: Offset) -> Option<&mut [u32; N]> {
        self.0.as_mut().split_first_mut().and_then(|(len, data)| {
            let begin = addr as usize;
            let end = begin + N;
            if end <= *len as usize {
                data.get_mut(begin..end)
                    .and_then(|block| block.try_into().ok())
            } else {
                None
            }
        })
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
    pub fn len(&self) -> Option<Offset> {
        self.0.len()
    }

    pub fn get<const N: usize>(&self, offset: Offset) -> Option<[Word; N]> {
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
        // .ok_or(CoreError::StackUnderflow)
    }
}

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    pub fn set_len(&mut self, len: Word) -> Option<()> {
        self.0.init(len)
    }

    pub fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Option<Offset> {
        self.0.alloc(words)
    }

    pub fn push<const N: usize>(&mut self, words: [u32; N]) -> Option<()> {
        self.0.alloc(words).map(|_| ())
    }

    // pub fn replace_or_add_at<const N: usize>(
    //     &mut self,
    //     offset: Offset,
    //     words: [Word; N],
    // ) -> Result<(), CoreError> {
    //     self.0
    //         .split_first_mut()
    //         .and_then(|(len, data)| {
    //             len.checked_sub(offset).and_then(|size| {
    //                 let addr = *len as usize;
    //                 if size as usize >= N {
    //                     data.get_mut(addr - N..addr).map(|block| {
    //                         block
    //                             .iter_mut()
    //                             .zip(words.iter())
    //                             .for_each(|(slot, value)| {
    //                                 *slot = *value;
    //                             });
    //                     })
    //                 } else {
    //                     data.get_mut(addr..addr + N).map(|block| {
    //                         block
    //                             .iter_mut()
    //                             .zip(words.iter())
    //                             .for_each(|(slot, value)| {
    //                                 *slot = *value;
    //                             });
    //                         *len += N as u32;
    //                     })
    //                 }
    //             })
    //         })
    //         .ok_or(CoreError::OutOfMemory)
    // }

    pub fn pop<const N: usize>(&mut self) -> Option<[u32; N]> {
        self.0.split_first_mut().and_then(|(len, data)| {
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
    pub fn get(&self, symbol: Symbol) -> Option<[u32; 8]> {
        // Symbol IDs start at 1, so return None for symbol 0
        if symbol == 0 {
            return None;
        }

        let (_, data) = self.0.split_first()?;
        let symbols = data.len() / 9;

        // Check if symbol is out of bounds
        if symbol > *self.0 .0.as_ref().first()? {
            return None;
        }

        let offset = symbols + (symbol - 1) as usize * 8;

        // Ensure offset is within bounds
        if offset + 8 > data.len() {
            return None;
        }

        data.get(offset..offset + 8)
            .and_then(|bytes| bytes.try_into().ok())
    }
}

impl<T> SymbolTable<T>
where
    T: AsMut<[Word]>,
{
    pub fn init(&mut self) -> Option<()> {
        self.0.init(0)
    }

    pub fn get_or_insert(&mut self, sym: [u32; 8]) -> Option<Symbol> {
        let (count, data) = self.0.split_first_mut()?;

        let symbols = data.len() / 9;
        if symbols == 0 {
            return None;
        }

        let h = hash_u32x8(sym) as usize;
        let mut index = h % symbols;
        let mut probe = 0;

        loop {
            let pos = data.get(index).copied()?;
            if pos == 0 {
                *count += 1;
                let pos = *count;

                // If we can't get the slot, something is fundamentally wrong
                let slot = match data.get_mut(index) {
                    Some(s) => s,
                    None => return None, // This should never happen, but return None if it does
                };
                *slot = pos;

                let offset = symbols + (pos - 1) as usize * 8;
                let value = data.get_mut(offset..offset + 8)?;
                value.iter_mut().zip(sym.iter()).for_each(|(dst, src)| {
                    *dst = *src;
                });
                return Some(pos);
            } else {
                let offset = symbols + (pos - 1) as usize * 8;
                let value = data.get_mut(offset..offset + 8)?;

                // Fix for clippy::op_ref
                if value == sym {
                    return Some(pos);
                }
            }
            index = (index + 1) % symbols;
            probe += 1;
            if probe >= symbols {
                return None;
            }
        }
    }
}

// C O N T E X T

#[derive(Debug)]
pub struct Context<T>(Memory<T>);

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
    pub fn get(&self, symbol: Symbol) -> Result<[Word; 2], CoreError> {
        let (_, data) = self.0.split_first().ok_or(CoreError::BoundsCheckFailed)?;

        let capacity = data.len() / Self::ENTRY_SIZE;
        if capacity == 0 {
            return Err(CoreError::WordNotFound);
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

        Err(CoreError::WordNotFound)
    }
}

impl<T> Context<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self) -> Option<()> {
        self.0.init(0)
    }

    pub fn put(&mut self, symbol: Symbol, value: [Word; 2]) -> Option<()> {
        let (count, data) = self.0.split_first_mut()?;

        let capacity = data.len() / Self::ENTRY_SIZE;
        if capacity == 0 {
            return None;
        }

        let h = Self::hash_u32(symbol) as usize;
        let mut index = h % capacity;

        for _probe in 0..capacity {
            let offset = index * Self::ENTRY_SIZE;
            if data
                .get_mut(offset..offset + Self::ENTRY_SIZE)
                .and_then(|entry| {
                    entry.split_first_mut().and_then(|(cur, val)| {
                        if *cur == 0 {
                            *count += 1;
                            *cur = symbol;
                        }
                        if *cur == symbol {
                            val[0] = value[0];
                            val[1] = value[1];
                            Some(())
                        } else {
                            None
                        }
                    })
                })
                .is_some()
            {
                return Some(());
            }
            index = (index + 1) % capacity;
        }
        None
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
    pub fn get_block(&self, addr: Offset) -> Option<&[Word]> {
        self.0.get_block(addr)
    }

    pub fn get<const N: usize>(&self, addr: Offset) -> Option<[Word; N]> {
        self.0.get(addr)
    }
}

impl<T> Heap<T>
where
    T: AsMut<[Word]>,
{
    pub fn init(&mut self, reserve: u32) -> Option<()> {
        self.0.init(reserve)
    }

    pub fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Option<Offset> {
        self.0.alloc(words)
    }

    pub fn alloc_empty_block(&mut self, size: Offset) -> Option<(Offset, &mut [Word])> {
        self.0.alloc_empty_block(size)
    }

    pub fn alloc_block(&mut self, values: &[Word]) -> Option<Offset> {
        self.0.alloc_block(values)
    }

    pub fn get_block_mut(&mut self, addr: Offset) -> Option<&mut [Word]> {
        self.0.get_block_mut(addr)
    }

    pub fn put<const N: usize>(&mut self, addr: Offset, value: [Word; N]) -> Option<()> {
        self.0.put(addr, value)
    }

    pub fn get_mut<const N: usize>(&mut self, addr: Offset) -> Option<&mut [u32; N]> {
        self.0.get_mut(addr)
    }

    //

    pub fn alloc_context(&mut self, size: Offset) -> Option<Offset> {
        let (addr, data) =
            self.alloc_empty_block(size * (Context::<T>::ENTRY_SIZE as Offset) + 1)?;
        Context::new(data).init()?;
        Some(addr)
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

    #[test]
    fn test_symbol_table_init_get_insert() {
        // Create a buffer large enough for the symbol table (9 slots per symbol)
        let mut buffer = vec![0u32; 100];
        let mut table = SymbolTable::new(buffer.as_mut_slice());

        // Initialize the table
        table.init().expect("Failed to initialize symbol table");

        // Insert a new symbol
        let sym1 = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let symbol1 = table.get_or_insert(sym1).expect("Failed to insert symbol");

        // Verify we can retrieve it
        let retrieved = table.get(symbol1).expect("Failed to get inserted symbol");
        assert_eq!(
            retrieved, sym1,
            "Retrieved symbol doesn't match inserted symbol"
        );

        // Insert another symbol
        let sym2 = [10u32, 20, 30, 40, 50, 60, 70, 80];
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
            let sym = [i + 1, i + 2, i + 3, i + 4, i + 5, i + 6, i + 7, i + 8];
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
            table.get(999).is_none(),
            "Getting non-existent symbol should return None"
        );

        // Symbol index 0 should be invalid (symbols start at 1)
        assert!(
            table.get(0).is_none(),
            "Getting symbol with ID 0 should return None"
        );

        // Test empty buffer scenario - a buffer that's technically big enough but too small to be practical
        // A proper symbol table needs at least 1 slot in the hash table + space for the symbol
        let mut small_buffer = vec![0u32; 1]; // Header only, no space for symbols
        let mut small_table = SymbolTable::new(small_buffer.as_mut_slice());
        small_table
            .init()
            .expect("Failed to initialize small table");

        let sym = [1u32, 2, 3, 4, 5, 6, 7, 8];
        assert!(
            small_table.get_or_insert(sym).is_none(),
            "Inserting into table without enough space should return None"
        );
    }
}
