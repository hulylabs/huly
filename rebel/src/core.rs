// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};
use crate::hash::hash_u32x8;

// T A G

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    None = 0,
    Int = 1,
    Block = 2,
    InlineString = 3,
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
const TAG_INLINE_STRING: Word = Tag::InlineString as Word;
const TAG_WORD: Word = Tag::Word as Word;
const TAG_SET_WORD: Word = Tag::SetWord as Word;

impl From<Word> for Tag {
    fn from(word: Word) -> Self {
        match word {
            TAG_NONE => Tag::None,
            TAG_INT => Tag::Int,
            TAG_BLOCK => Tag::Block,
            TAG_INLINE_STRING => Tag::InlineString,
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
    fn new(tag: Tag, value: Word) -> Self {
        Self { tag, value }
    }

    pub fn as_str<H, Y, S, O>(&self, memory: &Memory<H, Y, S, O>) -> Option<&str>
    where
        H: AsRef<[Word]>,
    {
        match self.tag {
            Tag::InlineString => {
                let buf = memory.heap.peek::<8>(self.value)?;
                let buf = unsafe { std::mem::transmute::<_, &[u8; 32]>(buf) };
                let len = buf[0] as usize;
                buf.get(1..len + 1)
                    .map(|buf| unsafe { std::str::from_utf8_unchecked(buf) })
            }
            _ => Some("<not a string>"),
        }
    }

    pub fn tag(&self) -> Tag {
        self.tag
    }
}

impl From<Value> for [Word; 2] {
    fn from(value: Value) -> Self {
        [value.tag.into(), value.value]
    }
}

// B L O C K

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

    fn peek<const N: usize>(&self, addr: Offset) -> Option<&[u32; N]> {
        self.0.as_ref().split_first().and_then(|(len, data)| {
            let begin = addr as usize;
            let end = begin + N;
            if end > *len as usize {
                None
            } else {
                data.get(begin..end).and_then(|block| block.try_into().ok())
            }
        })
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

    fn alloc_block(&mut self, values: &[Word]) -> Option<Offset> {
        let len = values.len() as u32;
        self.alloc_n(len + 1).and_then(|(addr, data)| {
            data.split_first_mut().map(|(size, block)| {
                *size = len;
                block
                    .iter_mut()
                    .zip(values.iter())
                    .for_each(|(slot, value)| {
                        *slot = *value;
                    });
                addr
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

struct Stack<T>(T);

impl<T> Stack<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self) -> Option<u32> {
        self.0.as_ref().first().copied()
    }
}

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

    fn pop_all(&mut self, offset: Offset) -> Option<&[Word]> {
        let (len, data) = self.0.as_mut().split_first_mut()?;
        len.checked_sub(offset).and_then(|size| {
            let addr = offset as usize;
            data.get(addr..addr + size as usize).inspect(|_| {
                *len = offset;
            })
        })
    }
}

// S Y M B O L   T A B L E

fn inline_string(string: &str) -> Option<[u32; 8]> {
    let bytes = string.as_bytes();
    let len = bytes.len();
    if len < 32 {
        let mut buf = [0; 32];
        buf[0] = len as u8;
        buf[1..len + 1].copy_from_slice(bytes);
        Some(unsafe { std::mem::transmute(buf) })
    } else {
        None
    }
}

type Symbol = Offset;

struct SymbolTable<T>(T);

impl<T> SymbolTable<T>
where
    T: AsMut<[Word]>,
{
    fn get_or_insert_symbol<H>(&mut self, str: [u32; 8], heap: &mut Block<H>) -> Option<Symbol>
    where
        H: AsRef<[Word]> + AsMut<[Word]>,
    {
        self.0.as_mut().split_first_mut().and_then(|(count, data)| {
            let table_len = data.len() as u32;
            let h = hash_u32x8(&str);
            let mut index = h.checked_rem(table_len)?;

            for _probe in 0..table_len {
                let offset = data.get_mut(index as usize)?;
                let stored_offset = *offset;

                if stored_offset == 0 {
                    let address = heap.alloc(str)?;
                    *offset = address;
                    *count += 1;
                    return Some(address);
                }

                if &str == heap.peek(stored_offset)? {
                    return Some(stored_offset);
                }
                index = (index + 1).checked_rem(table_len)?;
            }
            None
        })
    }
}

// C O N T E X T

struct Context<T>(T);

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
    fn get(&self, symbol: Symbol) -> Option<Value> {
        self.0.as_ref().split_first().and_then(|(_, data)| {
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
    fn put(&mut self, symbol: Symbol, value: Value) -> Option<()> {
        self.0.as_mut().split_first_mut().and_then(|(count, data)| {
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

// M E M O R Y

pub struct Memory<H, Y, S, O> {
    ops: Stack<O>,
    stack: Stack<S>,
    heap: Block<H>,
    symbols: SymbolTable<Y>,
}

impl<H, Y, S, O> Memory<H, Y, S, O>
where
    S: AsMut<[Word]>,
{
    pub fn pop_values<'a>(&'a mut self) -> Option<impl Iterator<Item = Value> + 'a> {
        let data = self.stack.pop_all(0)?;
        let mut iter = data.iter();
        Some(std::iter::from_fn(move || {
            let tag = iter.next()?;
            let value = iter.next()?;
            Some(Value::new((*tag).into(), *value))
        }))
    }
}

type SimpleLayout<'a> = Memory<&'a mut [Word], &'a mut [Word], &'a mut [Word], &'a mut [Word]>;

pub fn init_memory(
    memory: &mut [Word],
    stack_size: usize,
    ops_size: usize,
    symbols_size: usize,
) -> Option<SimpleLayout> {
    let (rest, stack) = memory.split_at_mut_checked(memory.len() - stack_size)?;
    let (rest, ops) = rest.split_at_mut_checked(rest.len() - ops_size)?;
    let (symbols, heap) = rest.split_at_mut_checked(symbols_size)?;

    Some(SimpleLayout {
        symbols: SymbolTable(symbols),
        heap: Block(heap),
        ops: Stack(ops),
        stack: Stack(stack),
    })
}

// P A R S E  C O L L E C T O R

pub trait Collector {
    fn string(&mut self, string: &str) -> Option<()>;
    fn word(&mut self, kind: WordKind, word: &str) -> Option<()>;
    fn integer(&mut self, value: i32) -> Option<()>;
    fn begin_block(&mut self) -> Option<()>;
    fn end_block(&mut self) -> Option<()>;
}

impl<H, Y, S, O> Collector for Memory<H, Y, S, O>
where
    H: AsMut<[Word]> + AsRef<[Word]>,
    S: AsMut<[Word]> + AsRef<[Word]>,
    Y: AsMut<[Word]>,
    O: AsMut<[Word]>,
{
    fn string(&mut self, string: &str) -> Option<()> {
        println!("string: {:?}", string);
        let offset = self.heap.alloc(inline_string(string)?)?;
        self.stack.push([Tag::InlineString.into(), offset])
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Option<()> {
        let offset = self
            .symbols
            .get_or_insert_symbol(inline_string(word)?, &mut self.heap)?;
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
        let offset = self.heap.alloc_block(block_data)?;
        self.stack.push([Tag::Block.into(), offset])
    }
}
