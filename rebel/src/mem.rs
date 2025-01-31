// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};
use crate::hash::hash_u32x8;

// T A G

#[repr(u32)]
enum Tag {
    Int,
    Block,
    String,
    Word,
    SetWord,
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
    symbols: Offset,
    heap: Offset,
    stack: Offset,
    ops: Offset,
}

impl<T> Memory<T>
where
    T: AsRef<[Word]>,
{
    // fn len(&self, address: Offset) -> Option<usize> {
    //     self.data
    //         .as_ref()
    //         .get(address as usize)
    //         .map(|len| *len as usize)
    // }

    // fn slice_get(&self, address: Offset) -> Option<&[Word]> {
    //     let address = address as usize;
    //     let len = self.data.as_ref().get(address).copied()? as usize;
    //     self.data.as_ref().get(address + 1..address + len)
    // }

    // fn get_heap(&self) -> Option<Stack<&[Word]>> {
    //     self.slice_get(self.heap).map(Stack::new)
    // }
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
            symbols: 0,
            heap,
            stack,
            ops,
        };

        // mem.alloc(0, heap)?; // symbol table
        // mem.alloc(heap, heap_size)?;
        // mem.alloc(ops, Self::OPS_SIZE)?;
        // mem.alloc(stack, Self::STACK_SIZE)?;

        Some(mem)
    }

    // fn slice_get_mut(&mut self, address: Offset) -> Option<&mut [Word]> {
    //     let address = address as usize;
    //     let len = self.data.as_ref().get(address).copied()? as usize;
    //     self.data.as_mut().get_mut(address + 1..address + len)
    // }

    // fn alloc(&mut self, address: Offset, size: Offset) -> Option<()> {
    //     self.data
    //         .as_mut()
    //         .get_mut(address as usize)
    //         .map(|len| *len = size)
    // }

    // fn get_heap_mut(&mut self) -> Option<Stack<&mut [Word]>> {
    //     self.slice_get_mut(self.heap).map(Stack::new)
    // }

    // fn get_stack_mut(&mut self) -> Option<Stack<&mut [Word]>> {
    //     self.slice_get_mut(self.stack).map(Stack::new)
    // }

    // fn get_ops_mut(&mut self) -> Option<Stack<&mut [Word]>> {
    //     self.slice_get_mut(self.ops).map(Stack::new)
    // }

    // fn get_symbols_mut(&mut self) -> Option<SymbolTable<&mut [Word]>> {
    //     self.slice_get_mut(self.symbols).map(SymbolTable::new)
    // }

    pub fn layout(
        &mut self,
    ) -> Option<
        ((
            SymbolTable<&mut [Word]>,
            Stack<&mut [Word]>,
            Stack<&mut [Word]>,
            Stack<&mut [Word]>,
        )),
    > {
        let (rest, stack) = self
            .data
            .as_mut()
            .split_at_mut_checked(self.stack as usize)?;
        let (rest, ops) = rest.split_at_mut_checked(self.ops as usize)?;
        let (symbols, heap) = rest.split_at_mut_checked(self.heap as usize)?;
        Some((
            SymbolTable::new(symbols),
            Stack::new(heap),
            Stack::new(ops),
            Stack::new(stack),
        ))
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
    pub fn push<const N: usize>(&mut self, value: [Word; N]) -> Option<()> {
        let data = self.data.as_mut();
        let capacity = data.len();
        data.split_first_mut().and_then(|(size, slot)| {
            let len = *size as usize;
            if capacity - len < N {
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

    pub fn push_all(&mut self, values: &[Word]) -> Option<()> {
        let data = self.data.as_mut();
        let capacity = data.len();
        data.split_first_mut().and_then(|(size, slot)| {
            let len = *size as usize;
            let values_len = values.len();
            if capacity - len < values_len {
                None
            } else {
                *size += values_len as u32;
                slot.get_mut(len..len + values_len).map(|items| {
                    items
                        .iter_mut()
                        .zip(values.iter())
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

// S Y M B O L

pub struct InlineString {
    buf: [u32; 8],
}

impl InlineString {
    fn new(string: &str) -> Option<Self> {
        let bytes = string.as_bytes();
        let len = bytes.len();
        if len < 32 {
            let mut buf = [0; 8];
            for i in 0..len {
                buf[i / 4] |= (bytes[i] as u32) << ((i % 4) * 8);
            }
            Some(InlineString { buf })
        } else {
            None
        }
    }

    pub fn hash(&self) -> u32 {
        hash_u32x8(&self.buf)
    }
}

// S Y M B O L   T A B L E

pub struct SymbolTable<T> {
    data: T,
}

impl<T> SymbolTable<T>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    pub fn new(data: T) -> Self {
        Self { data }
    }

    pub fn get_or_insert_symbol(
        &mut self,
        str: InlineString,
        heap: &mut Stack<T>,
    ) -> Option<Offset> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(count, data)| {
                let table_len = data.len() as u32;
                let h = str.hash();
                let mut index = h.checked_rem(table_len)?;

                for _probe in 0..table_len {
                    let offset = data.get_mut(index as usize)?;
                    let stored_offset = *offset;

                    if stored_offset == 0 {
                        let address = heap.len()?;
                        heap.push(str.buf)?;
                        *offset = address;
                        *count = *count + 1;
                        return Some(address);
                    }

                    if str.buf == heap.peek(stored_offset)? {
                        return Some(stored_offset);
                    }
                    index = (index + 1).checked_rem(table_len)?;
                }
                None
            })
    }
}

// P A R S E  C O L L E C T O R

pub trait Collector {
    fn string(&mut self, string: &str) -> Option<()>;
    fn word(&mut self, kind: WordKind, word: &str) -> Option<()>;
    fn integer(&mut self, value: i32) -> Option<()>;
    fn begin_block(&mut self) -> Option<()>;
    fn end_block(&mut self) -> Option<()>;
}

pub struct ParseCollector<T> {
    ops: Stack<T>,
    stack: Stack<T>,
    heap: Stack<T>,
    symbols: SymbolTable<T>,
}

impl<T> ParseCollector<T> {
    pub fn new(layout: (SymbolTable<T>, Stack<T>, Stack<T>, Stack<T>)) -> Self {
        Self {
            symbols: layout.0,
            heap: layout.1,
            ops: layout.2,
            stack: layout.3,
        }
    }
}

impl<T> Collector for ParseCollector<T>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    fn string(&mut self, string: &str) -> Option<()> {
        println!("string: {:?}", string);
        let string = InlineString::new(string)?;
        let offset = self.heap.len()?;
        self.heap.push(string.buf)?;
        self.stack.push([Tag::String.into(), offset])
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Option<()> {
        let symbol = InlineString::new(word)?;
        let offset = self.symbols.get_or_insert_symbol(symbol, &mut self.heap)?;
        let tag = match kind {
            WordKind::Word => Tag::Word,
            WordKind::SetWord => Tag::SetWord,
        };
        self.stack.push([tag.into(), offset])
    }

    fn integer(&mut self, value: i32) -> Option<()> {
        self.stack.push([Tag::Int.into(), value as u32])
    }

    fn begin_block(&mut self) -> Option<()> {
        println!("begin_block");
        self.ops.push([self.stack.len()?])
    }

    fn end_block(&mut self) -> Option<()> {
        println!("end_block");
        let block = self.stack.pop_all(self.ops.pop::<1>()?[0])?;
        self.heap.push_all(block)
    }
}
