// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};

pub trait Heap: AsRef<[Word]> {
    fn get_block(&self, addr: Offset) -> Option<&[Word]> {
        let addr = addr as usize;
        let len = self.as_ref().get(addr).copied()? as usize;
        self.as_ref().get(addr + 1..addr + 1 + len)
    }
}

pub trait Alloc: AsMut<[Word]> {
    fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Option<Offset> {
        let (len, data) = self.as_mut().split_first_mut()?;
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
    }

    fn alloc_n(&mut self, words: u32) -> Option<(Offset, &mut [Word])> {
        let (len, data) = self.as_mut().split_first_mut()?;
        data.get_mut(*len as usize..(*len + words) as usize)
            .map(|data| (*len, data))
            .inspect(|_| *len += words)
    }

    fn alloc_block(&mut self, words: u32) -> Option<(Offset, &mut [Word])> {
        self.alloc_n(words + 1).and_then(|(addr, data)| {
            data.split_first_mut().map(|(len, block)| {
                *len = words;
                (addr, block)
            })
        })
    }

    fn get_block_mut(&mut self, addr: Offset) -> Option<&mut [Word]> {
        let addr = addr as usize;
        let len = self.as_mut().get(addr).copied()? as usize;
        self.as_mut().get_mut(addr + 1..addr + 1 + len)
    }
}

pub trait StackMut: Alloc {
    fn init(&mut self) -> Option<()> {
        self.as_mut().first_mut().map(|slot| *slot = 0)
    }

    fn push<const N: usize>(&mut self, words: [u32; N]) -> Option<()> {
        self.alloc(words).map(|_| ())
    }

    fn pop<const N: usize>(&mut self) -> Option<[u32; N]> {
        let (len, data) = self.as_mut().split_first_mut()?;
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
    }
}

//

impl Alloc for &mut [Word] {}
impl StackMut for &mut [Word] {}

pub struct Memory<'a, T> {
    data: T,
    stack: &'a mut [Word],
}

impl<'a, T> Memory<'a, T>
where
    T: StackMut,
{
    pub fn init(&'a mut self) -> Option<()> {
        self.data.alloc_block(10).map(|(addr, mut stack)| {
            stack.init();
            self.stack = stack;
        })
    }

    pub fn alloc_stack(&mut self, words: u32) -> Option<Offset> {
        self.data.alloc_block(words).map(|(addr, mut stack)| {
            stack.init();
            addr
        })
    }
}

pub fn test(memory: &mut Memory<&mut [Word]>) -> Option<Offset> {
    memory.alloc_stack(10)
}
