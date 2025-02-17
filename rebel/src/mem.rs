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

pub struct SymbolTable<T>(Memory<T>);

impl<T> SymbolTable<T> {
    pub fn new(data: T) -> Self {
        Self(Memory(data))
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
        // println!("[symbol table]: add {:?}", sym);

        let (count, data) = self.0.split_first_mut()?;

        const ENTRY_SIZE: usize = 9;
        let capacity = data.len() / ENTRY_SIZE;
        if capacity == 0 {
            return None;
        }

        let h = hash_u32x8(&sym) as usize;
        let mut index = h % capacity;

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
                return Some(symbol);
            }
            index = (index + 1) % capacity;
        }

        None
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
