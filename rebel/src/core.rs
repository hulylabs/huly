// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};
use crate::hash::hash_u32x8;
use std::slice::ChunksExact;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RebelError {
    #[error(transparent)]
    AnyError(#[from] anyhow::Error),
}

// T A G

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    None = 0,
    Int = 1,
    Block = 2,
    InlineString = 3,
    Word = 4,
    SetWord = 5,
    NativeFn = 6,
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
pub const TAG_INT: Word = Tag::Int as Word;
const TAG_BLOCK: Word = Tag::Block as Word;
const TAG_INLINE_STRING: Word = Tag::InlineString as Word;
const TAG_WORD: Word = Tag::Word as Word;
const TAG_SET_WORD: Word = Tag::SetWord as Word;
const TAG_NATIVE_FN: Word = Tag::NativeFn as Word;

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

#[derive(Debug)]
pub struct Value {
    tag: Tag,
    value: Word,
}

impl Value {
    fn new(tag: Tag, value: Word) -> Self {
        Self { tag, value }
    }

    pub fn as_str<H, Y, S>(&self, memory: &Memory<H, Y, S>) -> Option<&str>
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

    fn reserve(&mut self, words: u32) -> Option<(Offset, &mut [Word])> {
        let (len, data) = self.0.as_mut().split_first_mut()?;
        data.get_mut(*len as usize..(*len + words) as usize)
            .map(|data| (*len, data))
            .inspect(|_| *len += words)
    }

    fn alloc_empty(&mut self, len: Offset) -> Option<(Offset, &mut [Word])> {
        self.reserve(len + 1).and_then(|(addr, data)| {
            data.split_first_mut().map(|(size, block)| {
                *size = len;
                (addr, block)
            })
        })
    }

    fn alloc_block(&mut self, values: &[Word]) -> Option<Offset> {
        self.alloc_empty(values.len() as u32).map(|(addr, data)| {
            data.iter_mut()
                .zip(values.iter())
                .for_each(|(slot, value)| {
                    *slot = *value;
                });
            addr
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

    fn peek<const N: usize>(&self) -> Option<&[Word]> {
        let (len, data) = self.0.as_ref().split_first()?;
        len.checked_sub(N as u32).and_then(|offset| {
            let addr = offset as usize;
            data.get(addr..addr + N)
        })
    }
}

impl<T> Stack<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self) -> Option<()> {
        self.0.as_mut().first_mut().map(|slot| *slot = 0)
    }

    fn push_offset<const N: usize>(&mut self, words: [u32; N]) -> Option<Offset> {
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

    fn push<const N: usize>(&mut self, words: [u32; N]) -> Option<()> {
        self.push_offset(words).map(|_| ())
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
    fn init(&mut self) -> Option<()> {
        self.0.as_mut().first_mut().map(|slot| *slot = 0)
    }

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
    fn get(&self, symbol: Symbol) -> Option<[u32; 2]> {
        self.0.as_ref().split_first().and_then(|(_, data)| {
            let table_len = (data.len() / 3) as u32;
            let h = Self::hash_u32(symbol);
            let mut index = h.checked_rem(table_len)?;

            for _probe in 0..table_len {
                let entry_offset = (index * 3) as usize;
                let entry = data.get(entry_offset..entry_offset + 3)?;
                if entry[0] == symbol {
                    return Some([entry[1], entry[2]]);
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
    fn init(&mut self) -> Option<()> {
        self.0.as_mut().first_mut().map(|slot| *slot = 0)
    }

    fn put(&mut self, symbol: Symbol, value: [Word; 2]) -> Option<()> {
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
                    entry[1] = value[0];
                    entry[2] = value[1];
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

pub struct NativeFn {
    func: fn(stack: Stack<Box<[Word]>>, heap: Block<&mut [Word]>) -> Result<(), RebelError>,
    arity: u32,
}

pub struct Memory<H, Y, S> {
    stack: Stack<S>,
    heap: Block<H>,
    symbols: SymbolTable<Y>,
    natives: Vec<NativeFn>,
}

impl<H, Y, S> Memory<H, Y, S>
where
    H: AsMut<[Word]>,
    Y: AsMut<[Word]>,
    S: AsMut<[Word]>,
{
    fn init(&mut self) -> Option<()> {
        self.symbols.init()?;
        self.stack.init()?;

        self.heap.init(0)?;
        let (root_ctx, root_ctx_data) = self.heap.alloc_empty(128)?;
        Context(root_ctx_data).init()?;
        debug_assert_eq!(root_ctx, 0);

        Some(())
    }
}

type SimpleLayout<'a> = Memory<&'a mut [Word], &'a mut [Word], &'a mut [Word]>;

pub fn init_memory(
    memory: &mut [Word],
    stack_size: usize,
    symbols_size: usize,
) -> Option<SimpleLayout> {
    let (rest, stack) = memory.split_at_mut_checked(memory.len() - stack_size)?;
    let (symbols, heap) = rest.split_at_mut_checked(symbols_size)?;

    let mut mem = SimpleLayout {
        heap: Block(heap),
        symbols: SymbolTable(symbols),
        stack: Stack(stack),
        natives: Vec::new(),
    };

    mem.init()?;
    Some(mem)
}

// P A R S E  C O L L E C T O R

pub trait Collector {
    fn string(&mut self, string: &str) -> Option<()>;
    fn word(&mut self, kind: WordKind, word: &str) -> Option<()>;
    fn integer(&mut self, value: i32) -> Option<()>;
    fn begin_block(&mut self) -> Option<()>;
    fn end_block(&mut self) -> Option<()>;
}

// E V A L  C O N T E X T

pub struct EvalContext<'a, H, Y, S> {
    memory: &'a mut Memory<H, Y, S>,
    parse: Stack<Box<[Word]>>,
    ops: Stack<Box<[Word]>>,
}

impl<'a, H, Y, S> EvalContext<'a, H, Y, S>
where
    H: AsMut<[Word]>,
    S: AsMut<[Word]> + AsRef<[Word]>,
{
    pub fn new(memory: &'a mut Memory<H, Y, S>) -> Self {
        Self {
            memory,
            parse: Stack(Box::new([0; 256])),
            ops: Stack(Box::new([0; 256])),
        }
    }

    pub fn pop_parse<'b>(&'b mut self) -> Option<ChunksExact<'b, Word>> {
        self.parse.pop_all(0).map(|data| data.chunks_exact(2))
    }

    pub fn pop_stack<'b>(&'b mut self) -> Option<ChunksExact<'b, Word>> {
        self.memory
            .stack
            .pop_all(0)
            .map(|data| data.chunks_exact(2))
    }

    pub fn eval_parsed(&mut self) -> Option<()> {
        let mut root_ctx = self.memory.heap.get_block_mut(0).map(Context)?;
        let data = self.parse.pop_all(0)?;

        for chunk in data.chunks_exact(2) {
            let value = match chunk[0] {
                TAG_WORD => root_ctx.get(chunk[1])?,
                _ => [chunk[0], chunk[1]],
            };

            let sp = self.memory.stack.push_offset(value)?;

            match value[0] {
                TAG_NATIVE_FN => {
                    let native_fn = self.memory.natives.get(value[1] as usize)?;
                    self.ops.push([sp, native_fn.arity * 2])?;
                }
                TAG_SET_WORD => {
                    self.ops.push([sp, 2])?;
                }
                _ => {}
            }

            if let Some([bp, arity]) = self.ops.peek::<2>() {
                if sp == bp + arity {
                    let frame = self.memory.stack.pop_all(*bp)?;
                    let op: [Word; 2] = frame.get(0..2).and_then(|op| op.try_into().ok())?;
                    match op {
                        [TAG_SET_WORD, sym] => {
                            let value: [Word; 2] =
                                frame.get(2..4).and_then(|value| value.try_into().ok())?;
                            root_ctx.put(sym, value)?;
                        }
                        _ => {
                            return None;
                        }
                    }
                }
            }
        }
        Some(())
    }
}

impl<'a, H, Y, S> Collector for EvalContext<'a, H, Y, S>
where
    H: AsMut<[Word]> + AsRef<[Word]>,
    Y: AsMut<[Word]>,
{
    fn string(&mut self, string: &str) -> Option<()> {
        println!("string: {:?}", string);
        let offset = self.memory.heap.alloc(inline_string(string)?)?;
        self.parse.push([Tag::InlineString.into(), offset])
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Option<()> {
        let offset = self
            .memory
            .symbols
            .get_or_insert_symbol(inline_string(word)?, &mut self.memory.heap)?;
        let tag = match kind {
            WordKind::Word => Tag::Word,
            WordKind::SetWord => Tag::SetWord,
        };
        self.parse.push([tag.into(), offset])
    }

    fn integer(&mut self, value: i32) -> Option<()> {
        self.parse.push([Tag::Int.into(), value as u32])
    }

    fn begin_block(&mut self) -> Option<()> {
        println!("begin_block");
        self.ops.push([self.parse.len()?])
    }

    fn end_block(&mut self) -> Option<()> {
        println!("end_block");
        let block_data = self.parse.pop_all(self.ops.pop::<1>()?[0])?;
        let offset = self.memory.heap.alloc_block(block_data)?;
        self.parse.push([Tag::Block.into(), offset])
    }
}

// pub fn test(ctx: &mut EvalContext<&mut [Word], &mut [Word], &mut [Word]>) -> Option<()> {
//     ctx.eval_parsed()
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ParseError, Parser};

    #[test]
    fn test_eval_1() -> Result<(), ParseError> {
        let mut buf = vec![0; 0x10000].into_boxed_slice();
        let mut mem = init_memory(&mut buf, 256, 1024).ok_or(ParseError::MemoryError)?;

        let mut ctx = EvalContext::new(&mut mem);
        let mut parser = Parser::new("x: 5", &mut ctx);
        parser.parse()?;
        ctx.eval_parsed().unwrap();

        let mut ctx = EvalContext::new(&mut mem);
        let mut parser = Parser::new("x", &mut ctx);
        parser.parse()?;
        ctx.eval_parsed().unwrap();

        let stack = ctx.pop_stack().unwrap().collect::<Vec<_>>();

        assert_eq!(stack.len(), 1);

        assert_eq!(stack[0][0], TAG_INT);
        assert_eq!(stack[0][1], 5);

        Ok(())
    }
}
