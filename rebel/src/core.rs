// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};
use crate::hash::hash_u32x8;
use std::{array::TryFromSliceError, slice::ChunksExact};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RebelError {
    #[error("memory error")]
    MemoryError,
    #[error("function not found")]
    FunctionNotFound,
    #[error("internal error")]
    InternalError,
    #[error("string too long")]
    StringTooLong,
    #[error("out of memory")]
    OutOfMemory,
    #[error("stack overflow")]
    StackOverflow,
    #[error("stack underflow")]
    StackUnderflow,
    #[error("symbol table full")]
    SymbolTableFull,
    #[error("type error")]
    TypeError,
    #[error("bad arguments")]
    BadArguments,
    #[error("word not bound")]
    WordNotBound,
    #[error(transparent)]
    SliceError(#[from] TryFromSliceError),
    #[error(transparent)]
    OtherError(#[from] anyhow::Error),
    #[error(transparent)]
    ParseError(#[from] crate::parse::ParseError),
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

    pub fn as_str<H, Y, S>(&self, memory: &Memory<H, Y, S>) -> Result<&str, RebelError>
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
                    .ok_or(RebelError::MemoryError)
            }
            _ => Err(RebelError::TypeError),
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

pub struct Block<T>(T);

impl<T> Block<T>
where
    T: AsRef<[Word]>,
{
    fn get_block(&self, addr: Offset) -> Option<Block<&[Word]>> {
        let data = self.0.as_ref();
        let addr = addr as usize;
        let len = data.get(addr).copied()? as usize;
        let start = addr + 1;
        data.get(start..start + len).map(Block)
    }

    fn peek<const N: usize>(&self, addr: Offset) -> Result<&[u32; N], RebelError> {
        let (len, data) = self
            .0
            .as_ref()
            .split_first()
            .ok_or(RebelError::MemoryError)?;

        let begin = addr as usize;
        let end = begin + N;
        if end > *len as usize {
            Err(RebelError::StackUnderflow)
        } else {
            data.get(begin..end)
                .map(|block| block.try_into())
                .transpose()?
                .ok_or(RebelError::MemoryError)
        }
    }
}

impl<T> Block<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self, size: u32) -> Result<(), RebelError> {
        self.0
            .as_mut()
            .first_mut()
            .map(|slot| *slot = size)
            .ok_or(RebelError::MemoryError)
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

    fn alloc<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, RebelError> {
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
            .ok_or(RebelError::OutOfMemory)
    }

    fn reserve(&mut self, words: u32) -> Option<(Offset, &mut [Word])> {
        let (len, data) = self.0.as_mut().split_first_mut()?;
        data.get_mut(*len as usize..(*len + words) as usize)
            .map(|data| (*len, data))
            .inspect(|_| *len += words)
    }

    fn alloc_empty(&mut self, len: Offset) -> Result<(Offset, &mut [Word]), RebelError> {
        self.reserve(len + 1)
            .and_then(|(addr, data)| {
                data.split_first_mut().map(|(size, block)| {
                    *size = len;
                    (addr, block)
                })
            })
            .ok_or(RebelError::OutOfMemory)
    }

    fn alloc_block(&mut self, values: &[Word]) -> Result<Offset, RebelError> {
        self.alloc_empty(values.len() as u32).map(|(addr, data)| {
            data.iter_mut()
                .zip(values.iter())
                .for_each(|(slot, value)| {
                    *slot = *value;
                });
            addr
        })
    }

    fn get_block_mut(&mut self, addr: Offset) -> Result<&mut [Word], RebelError> {
        let addr = addr as usize;
        let len = self
            .0
            .as_mut()
            .get(addr)
            .copied()
            .ok_or(RebelError::MemoryError)? as usize;

        self.0
            .as_mut()
            .get_mut(addr + 1..addr + 1 + len)
            .ok_or(RebelError::MemoryError)
    }
}

//

struct Stack<T>(T);

impl<T> Stack<T>
where
    T: AsRef<[Word]>,
{
    fn len(&self) -> Result<u32, RebelError> {
        self.0
            .as_ref()
            .first()
            .copied()
            .ok_or(RebelError::MemoryError)
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
    fn init(&mut self) -> Result<(), RebelError> {
        self.0
            .as_mut()
            .first_mut()
            .map(|slot| *slot = 0)
            .ok_or(RebelError::MemoryError)
    }

    fn push_offset<const N: usize>(&mut self, words: [u32; N]) -> Result<Offset, RebelError> {
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
            .ok_or(RebelError::StackOverflow)
    }

    fn push<const N: usize>(&mut self, words: [u32; N]) -> Result<(), RebelError> {
        self.push_offset(words).map(|_| ())
    }

    fn pop<const N: usize>(&mut self) -> Result<[u32; N], RebelError> {
        let (len, data) = self
            .0
            .as_mut()
            .split_first_mut()
            .ok_or(RebelError::MemoryError)?;

        len.checked_sub(N as u32)
            .and_then(|new_len| {
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
            .ok_or(RebelError::StackUnderflow)
    }

    fn pop_all(&mut self, offset: Offset) -> Result<&[Word], RebelError> {
        let (len, data) = self
            .0
            .as_mut()
            .split_first_mut()
            .ok_or(RebelError::MemoryError)?;

        len.checked_sub(offset)
            .and_then(|size| {
                let addr = offset as usize;
                data.get(addr..addr + size as usize).inspect(|_| {
                    *len = offset;
                })
            })
            .ok_or(RebelError::StackUnderflow)
    }
}

// S Y M B O L   T A B L E

fn inline_string(string: &str) -> Result<[u32; 8], RebelError> {
    let bytes = string.as_bytes();
    let len = bytes.len();
    if len < 32 {
        let mut buf = [0; 32];
        buf[0] = len as u8;
        buf[1..len + 1].copy_from_slice(bytes);
        Ok(unsafe { std::mem::transmute(buf) })
    } else {
        Err(RebelError::StringTooLong)
    }
}

type Symbol = Offset;

struct SymbolTable<T>(T);

impl<T> SymbolTable<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self) -> Result<(), RebelError> {
        self.0
            .as_mut()
            .first_mut()
            .map(|slot| *slot = 0)
            .ok_or(RebelError::MemoryError)
    }

    fn get_or_insert_symbol<H>(
        &mut self,
        str: [u32; 8],
        heap: &mut Block<H>,
    ) -> Result<Symbol, RebelError>
    where
        H: AsRef<[Word]> + AsMut<[Word]>,
    {
        let (count, data) = self
            .0
            .as_mut()
            .split_first_mut()
            .ok_or(RebelError::MemoryError)?;

        let table_len = data.len() as u32;
        let h = hash_u32x8(&str);
        let mut index = h.checked_rem(table_len).ok_or(RebelError::InternalError)?;

        for _probe in 0..table_len {
            let offset = data
                .get_mut(index as usize)
                .ok_or(RebelError::MemoryError)?;
            let stored_offset = *offset;

            if stored_offset == 0 {
                let address = heap.alloc(str)?;
                *offset = address;
                *count += 1;
                return Ok(address);
            }

            if &str == heap.peek(stored_offset)? {
                return Ok(stored_offset);
            }
            index = (index + 1)
                .checked_rem(table_len)
                .ok_or(RebelError::InternalError)?;
        }
        Err(RebelError::SymbolTableFull)
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
    fn get(&self, symbol: Symbol) -> Result<[u32; 2], RebelError> {
        let (_, data) = self
            .0
            .as_ref()
            .split_first()
            .ok_or(RebelError::MemoryError)?;

        let table_len = (data.len() / 3) as u32;
        let h = Self::hash_u32(symbol);
        let mut index = h.checked_rem(table_len).ok_or(RebelError::InternalError)?;

        for _probe in 0..table_len {
            let entry_offset = (index * 3) as usize;
            let entry = data
                .get(entry_offset..entry_offset + 3)
                .ok_or(RebelError::MemoryError)?;

            if entry[0] == symbol {
                return Ok([entry[1], entry[2]]);
            }

            index = (index + 1)
                .checked_rem(table_len)
                .ok_or(RebelError::InternalError)?;
        }
        Err(RebelError::WordNotBound)
    }
}

impl<T> Context<T>
where
    T: AsMut<[Word]>,
{
    fn init(&mut self) -> Result<(), RebelError> {
        self.0
            .as_mut()
            .first_mut()
            .map(|slot| *slot = 0)
            .ok_or(RebelError::MemoryError)
    }

    fn put(&mut self, symbol: Symbol, value: [Word; 2]) -> Result<(), RebelError> {
        let (count, data) = self
            .0
            .as_mut()
            .split_first_mut()
            .ok_or(RebelError::MemoryError)?;

        let table_len = (data.len() / 3) as u32;
        let h = Self::hash_u32(symbol);
        let mut index = h.checked_rem(table_len).ok_or(RebelError::InternalError)?;

        for _probe in 0..table_len {
            let entry_offset = (index * 3) as usize;
            let entry = data
                .get_mut(entry_offset..entry_offset + 3)
                .ok_or(RebelError::MemoryError)?;

            let stored_symbol = entry[0];
            if stored_symbol == 0 || stored_symbol == symbol {
                entry[0] = symbol;
                entry[1] = value[0];
                entry[2] = value[1];
                *count += 1;
                return Ok(());
            }

            index = (index + 1)
                .checked_rem(table_len)
                .ok_or(RebelError::InternalError)?;
        }
        Err(RebelError::SymbolTableFull)
    }
}

// M E M O R Y

type NativeFn = fn(stack: &[Word], heap: Block<&mut [Word]>) -> Result<[Word; 2], RebelError>;

#[derive(Debug, Clone, Copy)]
struct FuncDescriptor {
    func: NativeFn,
    arity: u32,
}

pub struct Module {
    pub procs: &'static [(&'static str, NativeFn, u32)],
}

pub struct Memory<H, Y, S> {
    stack: Stack<S>,
    heap: Block<H>,
    symbols: SymbolTable<Y>,
    natives: Vec<FuncDescriptor>,
}

impl<H, Y, S> Memory<H, Y, S>
where
    H: AsMut<[Word]> + AsRef<[Word]>,
    Y: AsMut<[Word]>,
    S: AsMut<[Word]>,
{
    fn init(&mut self) -> Result<(), RebelError> {
        self.symbols.init()?;
        self.stack.init()?;

        self.heap.init(0)?;
        let (root_ctx, root_ctx_data) = self.heap.alloc_empty(128)?;
        Context(root_ctx_data).init()?;
        debug_assert_eq!(root_ctx, 0);

        Ok(())
    }

    pub fn load_module(&mut self, module: &Module) -> Result<(), RebelError> {
        for (symbol, proc, arity) in module.procs {
            let id = self.natives.len() as Word;
            self.natives.push(FuncDescriptor {
                func: *proc,
                arity: *arity,
            });

            let symbol = self
                .symbols
                .get_or_insert_symbol(inline_string(*symbol)?, &mut self.heap)?;

            let mut root_ctx = self.heap.get_block_mut(0).map(Context)?;
            root_ctx.put(symbol, [TAG_NATIVE_FN, id])?;
        }
        Ok(())
    }
}

type SimpleLayout<'a> = Memory<&'a mut [Word], &'a mut [Word], &'a mut [Word]>;

pub fn init_memory(
    memory: &mut [Word],
    stack_size: usize,
    symbols_size: usize,
) -> Result<SimpleLayout, RebelError> {
    let (rest, stack) = memory
        .split_at_mut_checked(memory.len() - stack_size)
        .ok_or(RebelError::MemoryError)?;

    let (symbols, heap) = rest
        .split_at_mut_checked(symbols_size)
        .ok_or(RebelError::MemoryError)?;

    let mut mem = SimpleLayout {
        heap: Block(heap),
        symbols: SymbolTable(symbols),
        stack: Stack(stack),
        natives: Vec::new(),
    };

    mem.init()?;
    Ok(mem)
}

// P A R S E  C O L L E C T O R

pub trait Collector {
    fn string(&mut self, string: &str) -> Result<(), RebelError>;
    fn word(&mut self, kind: WordKind, word: &str) -> Result<(), RebelError>;
    fn integer(&mut self, value: i32) -> Result<(), RebelError>;
    fn begin_block(&mut self) -> Result<(), RebelError>;
    fn end_block(&mut self) -> Result<(), RebelError>;
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

    pub fn pop_parse(&mut self) -> Result<ChunksExact<'_, Word>, RebelError> {
        self.parse.pop_all(0).map(|data| data.chunks_exact(2))
    }

    pub fn pop_stack(&mut self) -> Result<ChunksExact<'_, Word>, RebelError> {
        self.memory
            .stack
            .pop_all(0)
            .map(|data| data.chunks_exact(2))
    }

    pub fn eval_parsed(&mut self) -> Result<(), RebelError> {
        self.memory.stack.init()?;
        let data = self.parse.pop_all(0)?;

        for chunk in data.chunks_exact(2) {
            let value = match chunk[0] {
                TAG_WORD => {
                    let root_ctx = self.memory.heap.get_block_mut(0).map(Context)?;
                    root_ctx.get(chunk[1])?
                }
                _ => [chunk[0], chunk[1]],
            };

            let sp = self.memory.stack.push_offset(value)?;

            match value[0] {
                TAG_NATIVE_FN => {
                    let native_fn = self
                        .memory
                        .natives
                        .get(value[1] as usize)
                        .ok_or(RebelError::FunctionNotFound)?;
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
                    let op: [Word; 2] =
                        frame.get(0..2).ok_or(RebelError::MemoryError)?.try_into()?;
                    match op {
                        [TAG_SET_WORD, sym] => {
                            let mut root_ctx = self.memory.heap.get_block_mut(0).map(Context)?;
                            let value: [Word; 2] =
                                frame.get(2..4).ok_or(RebelError::MemoryError)?.try_into()?;
                            root_ctx.put(sym, value)?;
                            self.memory.stack.push(value)?;
                        }
                        [TAG_NATIVE_FN, func] => {
                            let native_fn = self
                                .memory
                                .natives
                                .get(func as usize)
                                .ok_or(RebelError::FunctionNotFound)?;
                            let stack = frame.get(2..).ok_or(RebelError::MemoryError)?;
                            let heap = self.memory.heap.0.as_mut();
                            let result = (native_fn.func)(stack, Block(heap))?;
                            self.memory.stack.push(result)?;
                        }
                        _ => {
                            return Err(RebelError::InternalError);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<H, Y, S> Collector for EvalContext<'_, H, Y, S>
where
    H: AsMut<[Word]> + AsRef<[Word]>,
    Y: AsMut<[Word]>,
{
    fn string(&mut self, string: &str) -> Result<(), RebelError> {
        println!("string: {:?}", string);
        let offset = self.memory.heap.alloc(inline_string(string)?)?;
        self.parse.push([Tag::InlineString.into(), offset])
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Result<(), RebelError> {
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

    fn integer(&mut self, value: i32) -> Result<(), RebelError> {
        self.parse.push([Tag::Int.into(), value as u32])
    }

    fn begin_block(&mut self) -> Result<(), RebelError> {
        println!("begin_block");
        self.ops.push([self.parse.len()?])
    }

    fn end_block(&mut self) -> Result<(), RebelError> {
        println!("end_block");
        let block_data = self.parse.pop_all(self.ops.pop::<1>()?[0])?;
        let offset = self.memory.heap.alloc_block(block_data)?;
        self.parse.push([Tag::Block.into(), offset])
    }
}

// pub fn test(
//     ctx: &mut EvalContext<&mut [Word], &mut [Word], &mut [Word]>,
// ) -> Result<(), RebelError> {
//     ctx.eval_parsed()
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::CORE_MODULE;
    use crate::parse::Parser;

    #[test]
    fn test_eval_1() -> Result<(), RebelError> {
        let mut buf = vec![0; 0x10000].into_boxed_slice();
        let mut mem = init_memory(&mut buf, 256, 1024)?;

        let mut ctx = EvalContext::new(&mut mem);
        let mut parser = Parser::new("x: 5", &mut ctx);
        parser.parse()?;
        ctx.eval_parsed()?;

        let mut ctx = EvalContext::new(&mut mem);
        let mut parser = Parser::new("x", &mut ctx);
        parser.parse()?;
        ctx.eval_parsed()?;

        let stack = ctx.pop_stack().unwrap().collect::<Vec<_>>();

        assert_eq!(stack.len(), 1);

        assert_eq!(stack[0][0], TAG_INT);
        assert_eq!(stack[0][1], 5);

        mem.load_module(&CORE_MODULE)?;

        let mut ctx = EvalContext::new(&mut mem);
        let mut parser = Parser::new("add 7 8", &mut ctx);
        parser.parse()?;
        ctx.eval_parsed()?;

        let stack = ctx.pop_stack().unwrap().collect::<Vec<_>>();

        assert_eq!(stack.len(), 1);

        assert_eq!(stack[0][0], TAG_INT);
        assert_eq!(stack[0][1], 15);

        Ok(())
    }
}
