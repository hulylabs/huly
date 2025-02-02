// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::hash::hash_u32x8;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("string too long")]
    StringTooLong,
    #[error("bounds check failed")]
    BoundsCheckFailed,
    #[error("symbol table full")]
    SymbolTableFull,
    #[error("internal error")]
    InternalError,
    #[error("out of memory")]
    OutOfMemory,
    #[error("word not found")]
    WordNotFound,
    #[error("stack overflow")]
    StackOverflow,
    #[error("stack underflow")]
    StackUnderflow,
}

pub type Word = u32;
pub type Offset = Word;
pub type Symbol = Offset;

// B L O C K

struct Block<T>(T);

impl<T> Block<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self) -> Option<Offset> {
        self.0.as_ref().first().copied()
    }

    fn split_first(&self) -> Option<(&u32, &[Word])> {
        self.0.as_ref().split_first()
    }

    // fn get_at<const N: usize>(&self, addr: Offset) -> Option<&[u32; N]> {
    //     self.0.as_ref().split_first().and_then(|(len, data)| {
    //         let begin = addr as usize;
    //         let end = begin + N;
    //         if end > *len as usize {
    //             None
    //         } else {
    //             data.get(begin..end).and_then(|block| block.try_into().ok())
    //         }
    //     })
    // }

    // fn get_block(&self, addr: Offset) -> Option<Block<&[Word]>> {
    //     let data = self.0.as_ref();
    //     let addr = addr as usize;
    //     let len = data.get(addr).copied()? as usize;
    //     let start = addr + 1;
    //     data.get(start..start + len).map(Block)
    // }
}

impl<T> Block<T>
where
    T: AsMut<[Word]>,
{
    fn split_first_mut(&mut self) -> Option<(&mut u32, &mut [Word])> {
        self.0.as_mut().split_first_mut()
    }

    fn init(&mut self, size: u32) -> Option<()> {
        self.0.as_mut().first_mut().map(|slot| *slot = size)
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

    fn get_block_mut(&mut self, addr: Offset) -> Option<&mut [Word]> {
        self.0.as_mut().get_mut(addr as usize..).and_then(|data| {
            data.split_first_mut()
                .and_then(|(len, block)| block.get_mut(..*len as usize))
        })
    }

    // fn get_at_mut<const N: usize>(&mut self, addr: Offset) -> Option<&[u32; N]> {
    //     self.0.as_mut().split_first().and_then(|(len, data)| {
    //         let begin = addr as usize;
    //         let end = begin + N;
    //         if end > *len as usize {
    //             None
    //         } else {
    //             data.get(begin..end).and_then(|block| block.try_into().ok())
    //         }
    //     })
    // }

    // fn poke<const N: usize>(&mut self, addr: Offset, words: [u32; N]) -> Option<()> {
    //     let (len, data) = self.0.as_mut().split_first_mut()?;
    //     let addr = addr as usize;
    //     if addr + N > *len as usize {
    //         None
    //     } else {
    //         data.get_mut(addr..addr + N).map(|block| {
    //             block
    //                 .iter_mut()
    //                 .zip(words.iter())
    //                 .for_each(|(slot, value)| {
    //                     *slot = *value;
    //                 });
    //         })
    //     }
    // }

    // fn reserve(&mut self, words: u32) -> Option<(Offset, &mut [Word])> {
    //     let (len, data) = self.0.as_mut().split_first_mut()?;
    //     data.get_mut(*len as usize..(*len + words) as usize)
    //         .map(|data| (*len, data))
    //         .inspect(|_| *len += words)
    // }

    // fn alloc_empty(&mut self, len: Offset) -> Result<(Offset, &mut [Word]), RebelError> {
    //     self.reserve(len + 1)
    //         .and_then(|(addr, data)| {
    //             data.split_first_mut().map(|(size, block)| {
    //                 *size = len;
    //                 (addr, block)
    //             })
    //         })
    //         .ok_or(RebelError::OutOfMemory)
    // }

    // fn alloc_block(&mut self, values: &[Word]) -> Result<Offset, RebelError> {
    //     self.alloc_empty(values.len() as u32).map(|(addr, data)| {
    //         data.iter_mut()
    //             .zip(values.iter())
    //             .for_each(|(slot, value)| {
    //                 *slot = *value;
    //             });
    //         addr
    //     })
    // }
}

// S T A C K

pub struct Stack<T>(Block<T>);

impl<T> Stack<T> {
    pub fn new(data: T) -> Self {
        Self(Block(data))
    }
}

impl<T> Stack<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self) -> Option<Offset> {
        self.0.len()
    }

    // fn peek<const N: usize>(&self) -> Option<&[Word]> {
    //     let (len, data) = self.0.as_ref().split_first()?;
    //     len.checked_sub(N as u32).and_then(|offset| {
    //         let addr = offset as usize;
    //         data.get(addr..addr + N)
    //     })
    // }
}

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self) -> Option<()> {
        self.0.init(0)
    }

    pub fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, MemoryError> {
        self.0.alloc(words).ok_or(MemoryError::StackOverflow)
    }

    pub fn push<const N: usize>(&mut self, words: [u32; N]) -> Result<(), MemoryError> {
        self.0.alloc(words).ok_or(MemoryError::StackOverflow)?;
        Ok(())
    }

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

    pub fn pop_all(&mut self, offset: Offset) -> Result<&[Word], MemoryError> {
        self.0
            .split_first_mut()
            .and_then(|(len, data)| {
                len.checked_sub(offset).and_then(|size| {
                    let addr = offset as usize;
                    data.get(addr..addr + size as usize).inspect(|_| {
                        *len = offset;
                    })
                })
            })
            .ok_or(MemoryError::StackUnderflow)
    }
}

// S Y M B O L   T A B L E

fn inline_string(string: &str) -> Result<[u32; 8], MemoryError> {
    let bytes = string.as_bytes();
    let len = bytes.len();
    if len < 32 {
        let mut buf = [0; 8];
        buf[0] = len as u32;
        for i in 0..len {
            let j = i + 1;
            buf[j / 4] |= (bytes[i] as u32) << ((j % 4) * 8);
        }
        Ok(buf)
    } else {
        Err(MemoryError::StringTooLong)
    }
}

pub struct SymbolTable<T>(Block<T>);

impl<T> SymbolTable<T>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    fn init(&mut self) -> Option<()> {
        self.0.init(0)
    }

    fn get_or_insert(&mut self, sym: [u32; 8]) -> Result<Symbol, MemoryError> {
        let (count, data) = self
            .0
            .split_first_mut()
            .ok_or(MemoryError::BoundsCheckFailed)?;

        const ENTRY_SIZE: usize = 33;
        let capacity = data.len() / ENTRY_SIZE;

        let h = hash_u32x8(&sym) as usize;
        let mut index = h.checked_rem(capacity).ok_or(MemoryError::InternalError)?;

        for _probe in 0..capacity {
            let offset = index * ENTRY_SIZE;
            if let Some(symbol) = data.get_mut(offset..offset + ENTRY_SIZE).and_then(|entry| {
                entry.split_first_mut().and_then(|(symbol, value)| {
                    if *symbol == 0 {
                        *count += 1;
                        *symbol = *count;

                        value.iter_mut().zip(sym.iter()).for_each(|(dst, src)| {
                            *dst = *src;
                        });

                        Some(*symbol)
                    } else {
                        if value == &sym {
                            Some(*symbol)
                        } else {
                            None
                        }
                    }
                })
            }) {
                return Ok(symbol);
            }
            index = (index + 1)
                .checked_rem(capacity)
                .ok_or(MemoryError::InternalError)?;
        }

        Err(MemoryError::SymbolTableFull)
    }
}

// C O N T E X T

pub struct Context<T>(Block<T>);

impl<T> Context<T> {
    pub fn new(data: T) -> Self {
        Self(Block(data))
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
    pub fn get(&self, symbol: Symbol) -> Result<[Word; 2], MemoryError> {
        let (_, data) = self.0.split_first().ok_or(MemoryError::BoundsCheckFailed)?;

        const ENTRY_SIZE: usize = 3;
        let capacity = data.len() / ENTRY_SIZE;

        let h = Self::hash_u32(symbol) as usize;
        let mut index = h.checked_rem(capacity).ok_or(MemoryError::InternalError)?;

        for _probe in 0..capacity {
            let offset = index * ENTRY_SIZE;
            if let Some(found) = data.get(offset..offset + ENTRY_SIZE).and_then(|entry| {
                entry.split_first().and_then(|(cur, val)| {
                    if *cur == symbol {
                        Some([val[0], val[1]])
                    } else {
                        None
                    }
                })
            }) {
                return Ok(found);
            }
            index = (index + 1)
                .checked_rem(capacity)
                .ok_or(MemoryError::InternalError)?;
        }

        Err(MemoryError::WordNotFound)
    }
}

impl<T> Context<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self) -> Option<()> {
        self.0.init(0)
    }

    pub fn put(&mut self, symbol: Symbol, value: [Word; 2]) -> Result<(), MemoryError> {
        let (count, data) = self
            .0
            .split_first_mut()
            .ok_or(MemoryError::BoundsCheckFailed)?;

        const ENTRY_SIZE: usize = 3;
        let capacity = data.len() / ENTRY_SIZE;

        let h = Self::hash_u32(symbol) as usize;
        let mut index = h.checked_rem(capacity).ok_or(MemoryError::InternalError)?;

        for _probe in 0..capacity {
            let offset = index * ENTRY_SIZE;
            if data
                .get_mut(offset..offset + ENTRY_SIZE)
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
                return Ok(());
            }
            index = (index + 1)
                .checked_rem(capacity)
                .ok_or(MemoryError::InternalError)?;
        }

        Err(MemoryError::SymbolTableFull)
    }
}

// H E A P

pub struct Heap<T>(Block<T>);

impl<T> Heap<T>
where
    T: AsRef<[Word]>,
{
    // fn len(&self) -> Option<Offset> {
    //     self.0.len()
    // }
}

impl<T> Heap<T>
where
    T: AsMut<[Word]>,
{
    pub fn get_block_mut(&mut self, addr: Offset) -> Option<&mut [Word]> {
        self.0.get_block_mut(addr)
    }
}

//

// pub fn test(table: &mut SymbolTable<&mut [Word]>, sym: [u32; 8]) -> Result<Symbol, MemoryError> {
//     table.get_or_insert(sym)
// }

// pub fn inline(string: &str) -> Result<[u32; 8], MemoryError> {
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

// pub fn pop_all<'a>(stack: &'a mut Stack<&mut [Word]>, addr: Offset) -> Option<&'a [Word]> {
//     stack.pop_all(addr)
// }
