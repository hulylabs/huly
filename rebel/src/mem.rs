// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Address, Word};

// M E M O R Y

pub struct Memory<T> {
    data: T,
    heap: Address,
    stack: Address,
    ops: Address,
}

impl<T> Memory<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self, address: Address) -> Option<usize> {
        self.data
            .as_ref()
            .get(address as usize)
            .map(|len| *len as usize)
    }

    fn slice_get(&self, address: Address) -> Option<&[Word]> {
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

    pub fn new(data: T, heap: Address) -> Option<Self> {
        let len = data.as_ref().len() as Address;
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

    fn slice_get_mut(&mut self, address: Address) -> Option<&mut [Word]> {
        let address = address as usize;
        let len = self.data.as_ref().get(address).copied()? as usize;
        self.data.as_mut().get_mut(address + 1..address + len)
    }

    fn alloc(&mut self, address: Address, size: Address) -> Option<()> {
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

    fn peek<const N: usize>(&self, offset: Address) -> Option<[Word; N]> {
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
    fn push<const N: usize>(&mut self, value: [Word; N]) -> Option<()> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(size, slot)| {
                let len = *size as usize;
                let remaining = slot.len() - len;
                if remaining < N {
                    None
                } else {
                    *size += N as u32;
                    slot.get_mut(len..len + N).map(|items| {
                        items
                            .iter_mut()
                            .zip(value.iter())
                            .for_each(|(slot, value)| {
                                *slot = *value;
                            })
                    })
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
}
