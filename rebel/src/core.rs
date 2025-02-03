// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::boot::core_package;
use crate::mem::{Block, Context, ContextError, Heap, Offset, Stack, Symbol, SymbolTable, Word};
use crate::parse::{Collector, Parser, WordKind};
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
    #[error("bad arguments")]
    BadArguments,
}

// V A L U E

pub enum Value {
    None,
    Int(i32),
}

impl Value {
    pub const TAG_NONE: Word = 0;
    pub const TAG_INT: Word = 1;
    pub const TAG_BLOCK: Word = 2;
    const TAG_NATIVE_FN: Word = 3;
    const TAG_INLINE_STRING: Word = 4;
    const TAG_WORD: Word = 5;
    const TAG_SET_WORD: Word = 6;
}

fn inline_string(string: &str) -> Result<[u32; 8], CoreError> {
    let bytes = string.as_bytes();
    let len = bytes.len();
    if len < 32 {
        let mut buf = [0; 8];
        buf[0] = len as u32;
        for (i, byte) in bytes.iter().enumerate() {
            let j = i + 1;
            buf[j / 4] |= (*byte as u32) << ((j % 4) * 8);
        }
        Ok(buf)
    } else {
        Err(CoreError::StringTooLong)
    }
}

// M O D U L E

type NativeFn<T> = fn(stack: &[Word], module: &mut Module<T>) -> Result<[Word; 2], CoreError>;

struct FuncDesc<T> {
    func: NativeFn<T>,
    arity: u32,
}

pub struct Module<T> {
    heap: Heap<T>,
    functions: Vec<FuncDesc<T>>,
}

impl<T> Module<T> {
    const NULL: Offset = 0;
    const SYMBOLS: Offset = 1;
    const CONTEXT: Offset = 2;

    fn get_func(&self, index: u32) -> Result<&FuncDesc<T>, CoreError> {
        self.functions
            .get(index as usize)
            .ok_or(CoreError::FunctionNotFound)
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    pub fn init(data: T) -> Result<Self, CoreError> {
        let mut heap = Heap::new(data);
        heap.init(3)?;

        let (symbols_addr, symbols_data) = heap.alloc_empty_block(1024)?;
        SymbolTable::new(symbols_data).init()?;
        let (context_addr, context_data) = heap.alloc_empty_block(1024)?;
        Context::new(context_data).init(Self::NULL)?;

        heap.put(0, [0xdeadbeef, symbols_addr, context_addr])?;

        let mut module = Self {
            heap,
            functions: Vec::new(),
        };

        core_package(&mut module)?;
        Ok(module)
    }

    pub fn add_native_fn(
        &mut self,
        name: &str,
        func: NativeFn<T>,
        arity: u32,
    ) -> Result<(), CoreError> {
        let index = self.functions.len() as u32;
        self.functions.push(FuncDesc { func, arity });
        let symbol = inline_string(name)?;
        let id = self.get_symbols_mut()?.get_or_insert(symbol)?;
        self.get_context_mut()?
            .put(id, [Value::TAG_NATIVE_FN, index])
    }

    pub fn eval(&mut self, block: &[Word]) -> Result<Box<[Word]>, CoreError> {
        let mut stack = Stack::new([0; 128]);
        let mut ops = Stack::new([0; 64]);

        let mut cur: Option<[Word; 2]> = None;

        for chunk in block.chunks_exact(2) {
            let value = match chunk[0] {
                Value::TAG_WORD => self.find_word_current(chunk[1])?,
                _ => [chunk[0], chunk[1]],
            };

            let mut sp = stack.alloc(value)?;

            if let Some(arity) = match value[0] {
                Value::TAG_NATIVE_FN => Some(self.get_func(value[1])?.arity * 2),
                Value::TAG_SET_WORD => Some(2),
                _ => None,
            } {
                if let Some(value) = cur {
                    ops.push(value)?;
                }
                cur = Some([sp, arity]);
            }

            while let Some([bp, arity]) = cur {
                if sp == bp + arity {
                    let frame = stack.pop_all(bp)?;
                    let result = match frame {
                        [Value::TAG_SET_WORD, sym, tag, val] => {
                            self.get_context_mut()
                                .and_then(|mut ctx| ctx.put(*sym, [*tag, *val]))?;
                            value
                        }
                        [Value::TAG_NATIVE_FN, func, ..] => {
                            let native_fn = self.get_func(*func)?;
                            let stack_fn = frame.get(2..).ok_or(CoreError::BoundsCheckFailed)?;
                            (native_fn.func)(stack_fn, self)?
                        }
                        _ => {
                            return Err(CoreError::InternalError);
                        }
                    };
                    sp = stack.alloc(result)?;
                    cur = ops.pop();
                } else {
                    break;
                }
            }
        }
        Ok(stack.pop_all(0)?.into())
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]>,
{
    pub fn get_block(&self, addr: Offset) -> Result<Block<&[Word]>, CoreError> {
        self.heap.get_block(addr).map(Block::new)
    }

    fn find_word_current(&self, symbol: Symbol) -> Result<[Word; 2], CoreError> {
        let current = self.heap.get::<1>(Self::CONTEXT).map(|[addr]| *addr)?;
        self.find_word(current, symbol)
    }

    fn find_word(&self, context_addr: Offset, symbol: Symbol) -> Result<[Word; 2], CoreError> {
        let mut addr = context_addr;
        while addr != Self::NULL {
            let context = self.heap.get_block(addr).map(Context::new)?;
            match context.get(symbol) {
                Ok(result) => return Ok(result),
                Err(ContextError::WordNotFound(parent)) => addr = parent,
                Err(ContextError::CoreError(err)) => return Err(err),
            }
        }
        Err(CoreError::WordNotFound)
    }
}

impl<T> Module<T>
where
    T: AsMut<[Word]>,
{
    fn get_symbols_mut(&mut self) -> Result<SymbolTable<&mut [Word]>, CoreError> {
        let addr = self.heap.get_mut::<1>(Self::SYMBOLS).map(|[addr]| *addr)?;
        self.heap.get_block_mut(addr).map(SymbolTable::new)
    }

    fn get_context_mut(&mut self) -> Result<Context<&mut [Word]>, CoreError> {
        let addr = self.heap.get_mut::<1>(Self::CONTEXT).map(|[addr]| *addr)?;
        self.heap.get_block_mut(addr).map(Context::new)
    }

    pub fn parse(&mut self, code: &str) -> Result<Box<[Word]>, CoreError> {
        let mut collector = ParseCollector::new(self);
        Parser::new(code, &mut collector).parse()?;
        Ok(collector.parse.pop_all(0)?.into())
    }
}

// P A R S E  C O L L E C T O R

struct ParseCollector<'a, T> {
    module: &'a mut Module<T>,
    parse: Stack<[Word; 64]>,
    ops: Stack<[Word; 32]>,
}

impl<'a, T> ParseCollector<'a, T> {
    fn new(module: &'a mut Module<T>) -> Self {
        Self {
            module,
            parse: Stack::new([0; 64]),
            ops: Stack::new([0; 32]),
        }
    }
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
        let symbol = inline_string(word)?;
        let id = self.module.get_symbols_mut()?.get_or_insert(symbol)?;
        let tag = match kind {
            WordKind::Word => Value::TAG_WORD,
            WordKind::SetWord => Value::TAG_SET_WORD,
        };
        self.parse.push([tag, id])
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

//

// pub fn parse(module: &mut Module<&mut [Word]>, str: &str) -> Result<Box<[Word]>, CoreError> {
//     module.parse(str)
// }

// pub fn eval(module: &mut Module<&mut [Word]>, code: &[Word]) -> Result<Box<[Word]>, CoreError> {
//     module.eval(code)
// }

//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitespace_1() -> Result<(), CoreError> {
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse("  \t\n  ")?;
        let result = module.eval(&parsed)?;
        assert_eq!(result.len(), 0);
        Ok(())
    }

    #[test]
    fn test_string_1() -> Result<(), CoreError> {
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(" \"hello\"  ")?;
        let result = module.eval(&parsed)?;
        assert_eq!(Value::TAG_INLINE_STRING, result[0]);
        Ok(())
    }

    #[test]
    fn test_word_1() -> Result<(), CoreError> {
        let input = "42 \"world\" x: 5 x\n ";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!(result.len(), 8);
        assert_eq!([Value::TAG_INT, 42], result[0..2]);
        assert_eq!([Value::TAG_INT, 5, Value::TAG_INT, 5], result[4..8]);
        Ok(())
    }

    #[test]
    fn test_add_1() -> Result<(), CoreError> {
        let input = "add 7 8";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!([Value::TAG_INT, 15], result[0..2]);
        Ok(())
    }

    #[test]
    fn test_add_2() -> Result<(), CoreError> {
        let input = "add 1 add 2 3";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!([Value::TAG_INT, 6], result[0..2]);
        Ok(())
    }

    #[test]
    fn test_add_3() -> Result<(), CoreError> {
        let input = "add add 3 4 5";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!([Value::TAG_INT, 12], result[0..2]);
        Ok(())
    }
}
