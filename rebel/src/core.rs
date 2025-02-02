// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::mem::{Context, Heap, Stack, SymbolTable, Word};
use crate::parse::{Collector, WordKind};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("function not found")]
    FunctionNotFound,
    #[error("internal error")]
    InternalError,
    #[error("string too long")]
    StringTooLong,
    #[error("bounds check failed")]
    BoundsCheckFailed,
    #[error("symbol table full")]
    SymbolTableFull,
    #[error("out of memory")]
    OutOfMemory,
    #[error("word not found")]
    WordNotFound,
    #[error("stack underflow")]
    StackUnderflow,
    #[error("unexpected character: `{0}`")]
    UnexpectedChar(char),
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("integer overflow")]
    IntegerOverflow,
}

// V A L U E

pub enum Value {
    None,
    Int(i32),
}

impl Value {
    const TAG_NONE: Word = 0;
    const TAG_INT: Word = 1;
    const TAG_WORD: Word = 2;
    const TAG_SET_WORD: Word = 3;
    const TAG_NATIVE_FN: Word = 4;
    const TAG_INLINE_STRING: Word = 5;
    const TAG_BLOCK: Word = 6;
}

fn inline_string(string: &str) -> Result<[u32; 8], CoreError> {
    let bytes = string.as_bytes();
    let len = bytes.len();
    if len < 32 {
        let mut buf = [0; 8];
        buf[0] = len as u32;
        for i in 0..len {
            let j = i + 1;
            buf[j / 4] |= (bytes[i] as u32) << ((j % 4) * 8);
        }
        Ok(buf)
    } else {
        Err(CoreError::StringTooLong)
    }
}

// M O D U L E

type NativeFn<T> =
    for<'a> fn(stack: &[Word], module: &'a mut Module<T>) -> Result<[Word; 2], CoreError>;

struct FuncDesc<T> {
    func: NativeFn<T>,
    arity: u32,
}

pub struct Module<T> {
    heap: Heap<T>,
    functions: Vec<FuncDesc<T>>,
}

impl<T> Module<T> {
    fn get_func(&self, index: u32) -> Result<&FuncDesc<T>, CoreError> {
        self.functions
            .get(index as usize)
            .ok_or(CoreError::FunctionNotFound)
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]>,
{
    fn get_context(&self) -> Result<Context<&[Word]>, CoreError> {
        self.heap
            .get_block(0)
            .map(Context::new)
            .ok_or(CoreError::BoundsCheckFailed)
    }
}

impl<T> Module<T>
where
    T: AsMut<[Word]>,
{
    pub fn init(data: T) -> Result<Self, CoreError> {
        let mut heap = Heap::new(data);
        heap.init(3)?;

        let (symbols_addr, symbols_data) = heap.alloc_empty_block(1024)?;
        SymbolTable::new(symbols_data).init()?;
        let (context_addr, context_data) = heap.alloc_empty_block(1024)?;
        Context::new(context_data).init()?;

        heap.put(0, [0xdeadbeef, symbols_addr, context_addr])?;

        Ok(Self {
            heap,
            functions: Vec::new(),
        })
    }

    fn get_symbols_mut(&mut self) -> Result<SymbolTable<&mut [Word]>, CoreError> {
        let addr = self.heap.get_mut::<1>(1).map(|[addr]| *addr)?;
        self.heap
            .get_block_mut(addr)
            .map(SymbolTable::new)
            .ok_or(CoreError::BoundsCheckFailed)
    }

    fn get_context_mut(&mut self) -> Result<Context<&mut [Word]>, CoreError> {
        let addr = self.heap.get_mut::<1>(2).map(|[addr]| *addr)?;
        self.heap
            .get_block_mut(addr)
            .map(Context::new)
            .ok_or(CoreError::BoundsCheckFailed)
    }

    pub fn eval(&mut self, block: &[Word]) -> Result<[Word; 2], CoreError> {
        let mut stack = Stack::new([0; 128]);
        let mut ops = Stack::new([0; 64]);

        let mut cur: Option<[Word; 2]> = None;

        for chunk in block.chunks_exact(2) {
            let value = match chunk[0] {
                Value::TAG_WORD => self.get_context_mut().and_then(|ctx| ctx.get(chunk[1]))?,
                _ => [chunk[0], chunk[1]],
            };

            let mut sp = stack.alloc(value)?;

            if let Some(arity) = match value[0] {
                Value::TAG_NATIVE_FN => Some(self.get_func(value[1])?.arity * 2),
                Value::TAG_SET_WORD => Some(2),
                _ => None,
            } {
                if let Some(c) = cur {
                    ops.push(c)?;
                }
                cur = Some([sp, arity]);
            }

            while let Some([bp, arity]) = cur {
                if sp == bp + arity {
                    let frame = stack.pop_all(bp)?;
                    match frame {
                        [Value::TAG_SET_WORD, sym, tag, val] => {
                            self.get_context_mut()
                                .and_then(|mut ctx| ctx.put(*sym, [*tag, *val]))?;
                            sp = stack.alloc(value)?;
                        }
                        [Value::TAG_NATIVE_FN, func, ..] => {
                            let native_fn = self.get_func(*func)?;
                            let stack_fn = frame.get(2..).ok_or(CoreError::BoundsCheckFailed)?;
                            let result = (native_fn.func)(stack_fn, self)?;
                            sp = stack.alloc(result)?;
                        }
                        _ => {
                            return Err(CoreError::InternalError);
                        }
                    }
                    cur = ops.pop();
                } else {
                    break;
                }
            }
        }
        if let Some(value) = stack.pop() {
            Ok(value)
        } else {
            Ok([Value::TAG_NONE, 0])
        }
    }
}

// P A R S E  C O L L E C T O R

struct ParseCollector<'a, T> {
    module: &'a mut Module<T>,
    parse: Stack<[Word; 64]>,
    ops: Stack<[Word; 32]>,
}

impl<T> Collector for ParseCollector<'_, T>
where
    T: AsMut<[Word]>,
{
    fn string(&mut self, string: &str) -> Result<(), CoreError> {
        let offset = self.module.heap.alloc(inline_string(string)?)?;
        self.parse.push([Value::TAG_INLINE_STRING, offset])
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Result<(), CoreError> {
        let offset = self
            .module
            .get_symbols_mut()?
            .get_or_insert(inline_string(word)?)?;
        let tag = match kind {
            WordKind::Word => Value::TAG_WORD,
            WordKind::SetWord => Value::TAG_SET_WORD,
        };
        self.parse.push([tag, offset])
    }

    fn integer(&mut self, value: i32) -> Result<(), CoreError> {
        self.parse.push([Value::TAG_INT, value as u32])
    }

    fn begin_block(&mut self) -> Result<(), CoreError> {
        self.ops.push([self.parse.len()?])
    }

    fn end_block(&mut self) -> Result<(), CoreError> {
        let [bp] = self.ops.pop().ok_or(CoreError::StackUnderflow)?;
        let block_data = self.parse.pop_all(bp)?;
        let offset = self.module.heap.alloc_block(block_data)?;
        self.parse.push([Value::TAG_BLOCK, offset])
    }
}

pub fn eval(module: &mut Module<&mut [Word]>, block: &[Word]) -> Result<[Word; 2], CoreError> {
    module.eval(block)
}
