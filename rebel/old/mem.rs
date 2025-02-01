// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};
use crate::hash::hash_u32x8;

// T A G

enum Tag {
    None = 0,
    Int = 1,
    Block = 2,
    String = 3,
    Word = 4,
    SetWord = 5,
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

const TAG_NONE: Word = Tag::None as Word;
const TAG_INT: Word = Tag::Int as Word;
const TAG_BLOCK: Word = Tag::Block as Word;
const TAG_STRING: Word = Tag::String as Word;
const TAG_WORD: Word = Tag::Word as Word;
const TAG_SET_WORD: Word = Tag::SetWord as Word;

impl From<Word> for Tag {
    fn from(word: Word) -> Self {
        match word {
            TAG_NONE => Tag::None,
            TAG_INT => Tag::Int,
            TAG_BLOCK => Tag::Block,
            TAG_STRING => Tag::String,
            TAG_WORD => Tag::Word,
            TAG_SET_WORD => Tag::SetWord,
            _ => Tag::None,
        }
    }
}

// V A L U E

pub struct Value {
    tag: Tag,
    value: Word,
}

impl Value {
    pub fn new(tag: Tag, value: Word) -> Self {
        Self { tag, value }
    }
}

impl From<Value> for [Word; 2] {
    fn from(value: Value) -> Self {
        [value.tag.into(), value.value]
    }
}

// M E M O R Y

pub struct MemoryLayout<T> {
    ops: Stack<T>,
    stack: Stack<T>,
    heap: Heap<T>,
    symbols: SymbolTable<T>,
}

pub type MemoryMut<'a> = MemoryLayout<&'a mut [Word]>;

pub struct Memory<T> {
    data: T,
    heap: Offset,
    stack: Offset,
    ops: Offset,
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
        // let heap_size = ops.checked_sub(heap)?;

        Some(Self {
            data,
            heap,
            stack,
            ops,
        })
    }

    pub fn layout(&mut self) -> Option<MemoryLayout<&mut [Word]>> {
        let (rest, stack) = self
            .data
            .as_mut()
            .split_at_mut_checked(self.stack as usize)?;
        let (rest, ops) = rest.split_at_mut_checked(self.ops as usize)?;
        let (symbols, heap) = rest.split_at_mut_checked(self.heap as usize)?;
        Some(MemoryLayout {
            symbols: SymbolTable::new(symbols),
            heap: Heap::new(heap),
            ops: Stack::new(ops),
            stack: Stack::new(stack),
        })
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
        self.data.as_ref().first().copied()
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

pub type Symbol = Offset;

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
        heap: &mut Heap<T>,
    ) -> Option<Symbol> {
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
                        *count += 1;
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

// C O N T E X T

pub struct Context<T> {
    data: T,
}

impl<T> Context<T> {
    fn hash_u32(val: u32) -> u32 {
        const GOLDEN_RATIO: u32 = 0x9E3779B9;
        val.wrapping_mul(GOLDEN_RATIO)
    }
}

impl<T> Context<T>
where
    T: AsRef<[Word]>,
{
    pub fn new(data: T) -> Self {
        Self { data }
    }

    pub fn get(&self, symbol: Symbol) -> Option<Value> {
        self.data.as_ref().split_first().and_then(|(_, data)| {
            let table_len = (data.len() / 3) as u32;
            let h = Self::hash_u32(symbol);
            let mut index = h.checked_rem(table_len)?;

            for _probe in 0..table_len {
                let entry_offset = (index * 3) as usize;
                let entry = data.get(entry_offset..entry_offset + 3)?;
                if entry[0] == symbol {
                    return Some(Value::new(entry[1].into(), entry[2]));
                }

                index = (index + 1).checked_rem(table_len)?;
            }
            None
        })
    }
}

impl<T> Context<T>
where
    T: AsMut<[Word]>,
{
    pub fn put(&mut self, symbol: Symbol, value: Value) -> Option<()> {
        self.data
            .as_mut()
            .split_first_mut()
            .and_then(|(count, data)| {
                let table_len = (data.len() / 3) as u32;
                let h = Self::hash_u32(symbol);
                let mut index = h.checked_rem(table_len)?;

                for _probe in 0..table_len {
                    let entry_offset = (index * 3) as usize;
                    let entry = data.get_mut(entry_offset..entry_offset + 3)?;
                    let stored_symbol = entry[0];

                    if stored_symbol == 0 || stored_symbol == symbol {
                        entry[0] = symbol;
                        entry[1] = value.tag.into();
                        entry[2] = value.value;
                        *count += 1;
                        return Some(());
                    }

                    index = (index + 1).checked_rem(table_len)?;
                }
                None
            })
    }
}

// H E A P

pub struct Heap<T> {
    data: T,
}

impl<T> Heap<T>
where
    T: AsRef<[Word]>,
{
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T> Heap<T>
where
    T: AsMut<[Word]>,
{
    fn reserve(&mut self, size: Offset) -> Option<(Offset, &mut [Word])> {
        let (bump_pointer, data) = self.data.as_mut().split_first_mut()?;
        let addr = *bump_pointer as usize;
        let size = size + 1; // allocate one more word for block length
        let len = size as usize;
        data.get_mut(addr..addr + len)
            .and_then(|block| block.split_first_mut())
            .map(|(block_len, block_data)| {
                *block_len = size;
                *bump_pointer += size;
                // we return offset in `Self::data` slice, wich is one bump pointer more. This is also important to avoid zero pointer values.
                ((addr + 1) as Offset, block_data)
            })
    }

    pub fn alloc_block(&mut self, size: Offset) -> Option<Value> {
        let (addr, data) = self.reserve(size)?;
        data.first_mut().map(|len| *len = 0)?;
        Some(Value::new(Tag::Block, addr))
    }

    pub fn alloc<const N: usize>(&mut self, hash: [u32; N]) -> Option<Offset> {
        let (bump_pointer, data) = self.data.as_mut().split_first_mut()?;
        let addr = *bump_pointer as usize;
        data.get_mut(addr..addr + 8).map(|block| {
            block.iter_mut().zip(hash.iter()).for_each(|(slot, value)| {
                *slot = *value;
            });
            *bump_pointer += 8;
            addr as Offset
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

impl<T> Collector for MemoryLayout<T>
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
        let block_data = self.stack.pop_all(self.ops.pop::<1>()?[0])?;
        self.heap.push_all(block_data)
    }
}
