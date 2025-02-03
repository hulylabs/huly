// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::CoreError;
use crate::hash::hash_u32x8;

pub type Word = u32;
pub type Offset = Word;
pub type Symbol = Offset;

// O P S

#[derive(Debug)]
struct Ops<T>(T);

impl<T> Ops<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self) -> Result<Offset, CoreError> {
        self.0
            .as_ref()
            .first()
            .copied()
            .ok_or(CoreError::BoundsCheckFailed)
    }

    fn split_at<const M: usize>(&self) -> Option<([Word; M], &[Word])> {
        let (header, rest) = self.0.as_ref().split_at(M);
        header.try_into().ok().map(|header| (header, rest))
    }

    fn get_block(&self, addr: Offset) -> Result<&[Word], CoreError> {
        self.0
            .as_ref()
            .get(addr as usize + 1..)
            .and_then(|data| {
                data.split_first()
                    .and_then(|(len, block)| block.get(..*len as usize))
            })
            .ok_or(CoreError::BoundsCheckFailed)
    }

    fn get<const N: usize>(&self, addr: Offset) -> Option<&[u32; N]> {
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

impl<T> Ops<T>
where
    T: AsMut<[Word]>,
{
    fn split_first_mut(&mut self) -> Option<(&mut u32, &mut [Word])> {
        self.0.as_mut().split_first_mut()
    }

    fn split_at_mut<const M: usize>(&mut self) -> Option<(&mut [Word; M], &mut [Word])> {
        let (header, rest) = self.0.as_mut().split_at_mut(M);
        header.try_into().ok().map(|header| (header, rest))
    }

    fn init<const N: usize>(&mut self, values: [Word; N]) -> Result<(), CoreError> {
        self.0
            .as_mut()
            .first_chunk_mut()
            .map(|slot| *slot = values)
            .ok_or(CoreError::BoundsCheckFailed)
    }

    fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, CoreError> {
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
            .ok_or(CoreError::OutOfMemory)
    }

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

    fn get_block_mut(&mut self, addr: Offset) -> Result<&mut [Word], CoreError> {
        self.0
            .as_mut()
            .get_mut(addr as usize + 1..)
            .and_then(|data| {
                data.split_first_mut()
                    .and_then(|(len, block)| block.get_mut(..*len as usize))
            })
            .ok_or(CoreError::BoundsCheckFailed)
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

#[derive(Debug)]
pub struct Block<T>(Ops<T>);

impl<T> Block<T> {
    pub fn new(data: T) -> Self {
        Self(Ops(data))
    }
}

impl<T> Block<T>
where
    T: AsRef<[Word]>,
{
    pub fn as_ref(&self) -> &[Word] {
        self.0 .0.as_ref()
    }
}

// S T A C K

pub struct Stack<T>(Ops<T>);

impl<T> Stack<T> {
    pub fn new(data: T) -> Self {
        Self(Ops(data))
    }
}

impl<T> Stack<T>
where
    T: AsRef<[Word]>,
{
    pub fn len(&self) -> Result<Offset, CoreError> {
        self.0.len()
    }
}

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    pub fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, CoreError> {
        self.0.alloc(words)
    }

    pub fn push<const N: usize>(&mut self, words: [u32; N]) -> Result<(), CoreError> {
        self.0.alloc(words)?;
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

    pub fn pop_all(&mut self, offset: Offset) -> Result<&[Word], CoreError> {
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
            .ok_or(CoreError::StackUnderflow)
    }
}

// S Y M B O L   T A B L E

pub struct SymbolTable<T>(Ops<T>);

impl<T> SymbolTable<T> {
    pub fn new(data: T) -> Self {
        Self(Ops(data))
    }
}

impl<T> SymbolTable<T>
where
    T: AsMut<[Word]>,
{
    pub fn init(&mut self) -> Result<(), CoreError> {
        self.0.init([0])
    }

    pub fn get_or_insert(&mut self, sym: [u32; 8]) -> Result<Symbol, CoreError> {
        let (count, data) = self
            .0
            .split_first_mut()
            .ok_or(CoreError::BoundsCheckFailed)?;

        const ENTRY_SIZE: usize = 9;
        let capacity = data.len() / ENTRY_SIZE;

        let h = hash_u32x8(&sym) as usize;
        let mut index = h.checked_rem(capacity).ok_or(CoreError::InternalError)?;

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
                    } else if value == sym {
                        Some(*symbol)
                    } else {
                        None
                    }
                })
            }) {
                return Ok(symbol);
            }
            index = (index + 1)
                .checked_rem(capacity)
                .ok_or(CoreError::InternalError)?;
        }

        Err(CoreError::SymbolTableFull)
    }
}

// C O N T E X T

pub enum ContextError {
    CoreError(CoreError),
    WordNotFound(Offset),
}

#[derive(Debug)]
pub struct Context<T>(Ops<T>);

impl<T> Context<T> {
    pub fn new(data: T) -> Self {
        Self(Ops(data))
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
    pub fn get(&self, symbol: Symbol) -> Result<[Word; 2], ContextError> {
        let ([parent, _], data) = self
            .0
            .split_at()
            .ok_or(ContextError::CoreError(CoreError::BoundsCheckFailed))?;

        const ENTRY_SIZE: usize = 3;
        let capacity = data.len() / ENTRY_SIZE;

        let h = Self::hash_u32(symbol) as usize;
        let mut index = h
            .checked_rem(capacity)
            .ok_or(ContextError::CoreError(CoreError::InternalError))?;

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
                .ok_or(ContextError::CoreError(CoreError::InternalError))?;
        }

        Err(ContextError::WordNotFound(parent))
    }
}

impl<T> Context<T>
where
    T: AsMut<[Word]>,
{
    pub fn init(&mut self, parent: Offset) -> Result<(), CoreError> {
        self.0.init([parent, 0])
    }

    pub fn put(&mut self, symbol: Symbol, value: [Word; 2]) -> Result<(), CoreError> {
        let ([_, count], data) = self.0.split_at_mut().ok_or(CoreError::BoundsCheckFailed)?;

        const ENTRY_SIZE: usize = 3;
        let capacity = data.len() / ENTRY_SIZE;

        let h = Self::hash_u32(symbol) as usize;
        let mut index = h.checked_rem(capacity).ok_or(CoreError::InternalError)?;

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
                .ok_or(CoreError::InternalError)?;
        }

        Err(CoreError::SymbolTableFull)
    }
}

// H E A P

pub struct Heap<T>(Ops<T>);

impl<T> Heap<T> {
    pub fn new(data: T) -> Self {
        Self(Ops(data))
    }
}

impl<T> Heap<T>
where
    T: AsRef<[Word]>,
{
    pub fn get_block(&self, addr: Offset) -> Result<&[Word], CoreError> {
        self.0.get_block(addr)
    }

    pub fn get<const N: usize>(&self, addr: Offset) -> Result<&[u32; N], CoreError> {
        self.0.get(addr).ok_or(CoreError::BoundsCheckFailed)
    }
}

impl<T> Heap<T>
where
    T: AsMut<[Word]>,
{
    pub fn init(&mut self, reserve: u32) -> Result<(), CoreError> {
        self.0.init([reserve])
    }

    pub fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, CoreError> {
        self.0.alloc(words)
    }

    pub fn alloc_empty_block(&mut self, size: Offset) -> Result<(Offset, &mut [Word]), CoreError> {
        self.0.alloc_empty_block(size).ok_or(CoreError::OutOfMemory)
    }

    pub fn alloc_block(&mut self, values: &[Word]) -> Result<Offset, CoreError> {
        self.0.alloc_block(values).ok_or(CoreError::OutOfMemory)
    }

    pub fn get_block_mut(&mut self, addr: Offset) -> Result<&mut [Word], CoreError> {
        self.0.get_block_mut(addr)
    }

    pub fn put<const N: usize>(&mut self, addr: Offset, value: [Word; N]) -> Result<(), CoreError> {
        self.0.put(addr, value).ok_or(CoreError::BoundsCheckFailed)
    }

    pub fn get_mut<const N: usize>(&mut self, addr: Offset) -> Result<&mut [u32; N], CoreError> {
        self.0.get_mut(addr).ok_or(CoreError::BoundsCheckFailed)
    }
}

//

// pub fn test(table: &mut SymbolTable<&mut [Word]>, sym: [u32; 8]) -> Result<Symbol, CoreError> {
//     table.get_or_insert(sym)
// }

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

// pub fn pop_all<'a>(stack: &'a mut Stack<&mut [Word]>, addr: Offset) -> Option<&'a [Word]> {
//     stack.pop_all(addr)
// }
