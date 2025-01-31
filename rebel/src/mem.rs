// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};

// T A G

#[repr(u32)]
enum Tag {
    Int,
    Block,
}

pub enum WordKind {
    Word,
    SetWord,
}

impl From<Tag> for Word {
    fn from(tag: Tag) -> Self {
        tag as Word
    }
}

// M E M O R Y

pub struct Memory<T> {
    data: T,
    heap: Offset,
    stack: Offset,
    ops: Offset,
}

impl<T> Memory<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self, address: Offset) -> Option<usize> {
        self.data
            .as_ref()
            .get(address as usize)
            .map(|len| *len as usize)
    }

    fn slice_get(&self, address: Offset) -> Option<&[Word]> {
        let address = address as usize;
        let len = self.data.as_ref().get(address).copied()? as usize;
        self.data.as_ref().get(address + 1..address + len)
    }

    fn get_heap(&self) -> Option<Stack<&[Word]>> {
        self.slice_get(self.heap).map(Stack::new)
    }
}

impl<T> Memory<T>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    const STACK_SIZE: u32 = 1024;
    const OPS_SIZE: u32 = 256;

    pub fn new(data: T, heap: Offset) -> Option<Self> {
        let len = data.as_ref().len() as Offset;
        let stack = len.checked_sub(Self::STACK_SIZE)?;
        let ops = stack.checked_sub(Self::OPS_SIZE)?;
        let heap_size = ops.checked_sub(heap)?;

        let mut mem = Self {
            data,
            heap,
            stack,
            ops,
        };

        mem.alloc(heap, heap_size)?;
        mem.alloc(stack, Self::STACK_SIZE)?;
        mem.alloc(ops, Self::OPS_SIZE)?;

        Some(mem)
    }

    fn slice_get_mut(&mut self, address: Offset) -> Option<&mut [Word]> {
        let address = address as usize;
        let len = self.data.as_ref().get(address).copied()? as usize;
        self.data.as_mut().get_mut(address + 1..address + len)
    }

    fn alloc(&mut self, address: Offset, size: Offset) -> Option<()> {
        self.data
            .as_mut()
            .get_mut(address as usize)
            .map(|len| *len = size)
    }

    fn get_heap_mut(&mut self) -> Option<Stack<&mut [Word]>> {
        self.slice_get_mut(self.heap).map(Stack::new)
    }

    fn get_stack_mut(&mut self) -> Option<Stack<&mut [Word]>> {
        self.slice_get_mut(self.stack).map(Stack::new)
    }

    fn get_ops_mut(&mut self) -> Option<Stack<&mut [Word]>> {
        self.slice_get_mut(self.ops).map(Stack::new)
    }
}

// S T A C K

pub struct Stack<T> {
    data: T,
}

impl<T> Stack<T>
where
    T: AsRef<[Word]>,
{
    pub fn new(data: T) -> Self {
        Self { data }
    }

    pub fn len(&self) -> Option<Word> {
        self.data.as_ref().get(0).copied()
    }

    pub fn peek<const N: usize>(&self, offset: Offset) -> Option<[Word; N]> {
        let offset = offset as usize;
        self.data.as_ref().split_first().and_then(|(_, slot)| {
            slot.get(offset..offset + N)
                .and_then(|slot| slot.try_into().ok())
        })
    }
}

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    pub fn push<const N: usize>(&mut self, value: [Word; N]) -> Option<Offset> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(size, slot)| {
                let result = *size;
                let len = result as usize;
                let remaining = slot.len() - len;
                if remaining < N {
                    None
                } else {
                    slot.get_mut(len..len + N).map(|items| {
                        items
                            .iter_mut()
                            .zip(value.iter())
                            .for_each(|(slot, value)| {
                                *slot = *value;
                            })
                    })?;
                    *size += N as u32;
                    Some(result)
                }
            })
    }

    pub fn push_all(&mut self, values: &[Word]) -> Option<Offset> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(size, slot)| {
                let result = *size;
                let len = result as usize;
                let remaining = slot.len() - len;
                let values_len = values.len();
                if remaining < values_len {
                    None
                } else {
                    slot.get_mut(len..len + values_len).map(|items| {
                        items
                            .iter_mut()
                            .zip(values.iter())
                            .for_each(|(slot, value)| {
                                *slot = *value;
                            })
                    })?;
                    *size += values_len as u32;
                    Some(result)
                }
            })
    }

    fn pop<const N: usize>(&mut self) -> Option<[Word; N]> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(size, slot)| {
                size.checked_sub(N as u32).and_then(|sp| {
                    let len = sp as usize;
                    slot.get(len..len + N).map(|slot| {
                        let mut value = [0; N];
                        value.iter_mut().zip(slot.iter()).for_each(|(value, slot)| {
                            *value = *slot;
                        });
                        *size = sp;
                        value
                    })
                })
            })
    }

    fn pop_all(&mut self, offset: Offset) -> Option<&[Word]> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(size, slot)| {
                size.checked_sub(offset).and_then(|sp| {
                    let len = sp as usize;
                    *size = sp;
                    slot.get(len..len + offset as usize)
                })
            })
    }
}

// P A R S E  C O L L E C T O R

pub trait Collector {
    fn string(&self, string: &str) -> Option<()>;
    fn word(&self, kind: WordKind, word: &str) -> Option<()>;
    fn integer(&mut self, value: i32) -> Option<()>;
    fn begin_block(&mut self) -> Option<()>;
    fn end_block(&mut self) -> Option<()>;
}

pub struct ParseCollector<T> {
    ops: Stack<T>,
    stack: Stack<T>,
    heap: Stack<T>,
}

impl<T> ParseCollector<T>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    pub fn new(ops: Stack<T>, stack: Stack<T>, heap: Stack<T>) -> Self {
        Self { ops, stack, heap }
    }
}

impl<T> Collector for ParseCollector<T>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    fn string(&self, string: &str) -> Option<()> {
        unimplemented!()
    }

    fn word(&self, kind: WordKind, word: &str) -> Option<()> {
        unimplemented!()
    }

    fn integer(&mut self, value: i32) -> Option<()> {
        self.stack.push([Tag::Int.into(), value as u32])?;
        Some(())
    }

    fn begin_block(&mut self) -> Option<()> {
        self.ops.push([self.stack.len()?])?;
        Some(())
    }

    fn end_block(&mut self) -> Option<()> {
        let block = self.stack.pop_all(self.ops.pop::<1>()?[0])?;
        self.heap.push_all(block)?;
        Some(())
    }
}
