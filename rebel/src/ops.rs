// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};

struct Block<T>(T);

impl<T> Block<T>
where
    T: AsRef<[Word]>,
{
    fn get_block(&self, addr: Offset) -> Option<Block<&[Word]>> {
        let data = self.0.as_ref();
        let addr = addr as usize;
        let len = data.get(addr).copied()? as usize;
        let start = addr + 1;
        data.get(start..start + len).map(|block| Block(block))
    }
}

impl<T> Block<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self, size: u32) -> Option<()> {
        self.0.as_mut().first_mut().map(|slot| *slot = size)
    }

    fn poke<const N: usize>(&mut self, addr: Offset, words: [u32; N]) -> Option<()> {
        let (len, data) = self.0.as_mut().split_first_mut()?;
        let addr = addr as usize;
        if addr + N > *len as usize {
            None
        } else {
            data.get_mut(addr..addr + N).map(|block| {
                block
                    .iter_mut()
                    .zip(words.iter())
                    .for_each(|(slot, value)| {
                        *slot = *value;
                    });
            })
        }
    }

    fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Option<Offset> {
        let (len, data) = self.0.as_mut().split_first_mut()?;
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
        let (len, data) = self.0.as_mut().split_first_mut()?;
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
        let len = self.0.as_mut().get(addr).copied()? as usize;
        self.0.as_mut().get_mut(addr + 1..addr + 1 + len)
    }
}

//

pub struct Stack<T>(T);

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self) -> Option<()> {
        self.0.as_mut().first_mut().map(|slot| *slot = 0)
    }

    fn push<const N: usize>(&mut self, words: [u32; N]) -> Option<()> {
        let (len, data) = self.0.as_mut().split_first_mut()?;
        let addr = *len as usize;
        data.get_mut(addr..addr + N).map(|block| {
            block
                .iter_mut()
                .zip(words.iter())
                .for_each(|(slot, value)| {
                    *slot = *value;
                });
            *len += N as u32
        })
    }

    fn pop<const N: usize>(&mut self) -> Option<[u32; N]> {
        let (len, data) = self.0.as_mut().split_first_mut()?;
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

pub struct MemoryLayout {
    stack: Offset,
    ops: Offset,
}

impl MemoryLayout {
    const LAYOUT_SIZE: u32 = 2;
}

pub struct Memory<T>(Block<T>);

impl<T> Memory<T>
where
    T: AsMut<[Word]>,
{
    fn init(memory: T, stack_size: Offset, ops_size: Offset) -> Option<MemoryLayout> {
        let mut heap = Block(memory);
        heap.init(MemoryLayout::LAYOUT_SIZE)?;

        let stack = heap
            .alloc_block(stack_size)
            .and_then(|(addr, data)| Stack(data).init().map(|_| addr))?;
        let ops = heap
            .alloc_block(ops_size)
            .and_then(|(addr, data)| Stack(data).init().map(|_| addr))?;

        heap.poke(0, [stack, ops])?;
        Some(MemoryLayout { stack, ops })
    }
}

pub fn init(memory: &mut [Word]) -> Option<MemoryLayout> {
    Memory::init(memory, 256, 256)
}
