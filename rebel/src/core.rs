// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::boot::core_package;
use crate::mem::{Context, Heap, Offset, Stack, Symbol, SymbolTable, Word};
use crate::parse::{Collector, Parser, WordKind};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("internal error")]
    InternalError,
    #[error("function not found")]
    FunctionNotFound,
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
    pub const TAG_CONTEXT: Word = 3;
    pub const TAG_NATIVE_FN: Word = 4;
    pub const TAG_INLINE_STRING: Word = 5;
    pub const TAG_WORD: Word = 6;
    pub const TAG_SET_WORD: Word = 7;
    pub const TAG_STACK_VALUE: Word = 8;
    pub const TAG_FUNC: Word = 9;
    pub const TAG_BOOL: Word = 10;
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

type NativeFn<T> = fn(module: &mut Exec<T>, bp: Offset) -> Result<[Word; 2], CoreError>;

struct FuncDesc<T> {
    func: NativeFn<T>,
    arity: u32,
}

pub struct Module<T> {
    heap: Heap<T>,
    system_words: Offset,
    functions: Vec<FuncDesc<T>>,
}

impl<T> Module<T> {
    // const NULL: Offset = 0;
    const SYMBOLS: Offset = 1;
    // const CONTEXT: Offset = 2;

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

        let system_words = heap.alloc_context(1024)?;

        let mut module = Self {
            heap,
            system_words,
            functions: Vec::new(),
        };

        let (symbols_addr, symbols_data) = module.heap.alloc_empty_block(1024)?;
        SymbolTable::new(symbols_data).init()?;

        module
            .heap
            .put(0, [0xdeadbeef, symbols_addr, system_words])?;
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
        let mut words = self
            .heap
            .get_block_mut(self.system_words)
            .map(Context::new)?;
        words.put(id, [Value::TAG_NATIVE_FN, index])
    }

    pub fn eval(&mut self, block: &[Word]) -> Result<[Word; 2], CoreError> {
        Exec::new(self)?.eval(block.iter().copied())
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]>,
{
    fn get_array<const N: usize>(&self, addr: Offset) -> Result<[Word; N], CoreError> {
        self.heap.get(addr)
    }

    fn get_block(&self, addr: Offset) -> Result<Box<[Word]>, CoreError> {
        self.heap.get_block(addr).map(|block| block.into())
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

    pub fn parse(&mut self, code: &str) -> Result<Box<[Word]>, CoreError> {
        let mut collector = ParseCollector::new(self);
        Parser::new(code, &mut collector).parse()?;
        Ok(collector.parse.pop_all(0)?.into())
    }
}

// E X E C U T I O N  C O N T E X T

pub struct Exec<'a, T> {
    module: &'a mut Module<T>,
    stack: Stack<[Offset; 1024]>,
    ops: Stack<[Offset; 256]>,
    base: Stack<[Offset; 256]>,
    env: Stack<[Offset; 256]>,
}

impl<'a, T> Exec<'a, T> {
    fn new(module: &'a mut Module<T>) -> Result<Self, CoreError> {
        let mut env = Stack::new([0; 256]);
        env.push([module.system_words])?;
        Ok(Self {
            module,
            env,
            stack: Stack::new([0; 1024]),
            ops: Stack::new([0; 256]),
            base: Stack::new([0; 256]),
        })
    }
}

impl<'a, T> Exec<'a, T>
where
    T: AsRef<[Word]>,
{
    pub fn get_block(&self, addr: Offset) -> Result<Box<[Word]>, CoreError> {
        self.module.get_block(addr)
    }

    pub fn stack_get<const N: usize>(&self, bp: Offset) -> Result<[Word; N], CoreError> {
        self.stack.get(bp)
    }

    fn find_word(&self, symbol: Symbol) -> Result<[Word; 2], CoreError> {
        let [ctx] = self.env.peek().ok_or(CoreError::InternalError)?;
        let context = self.module.heap.get_block(ctx).map(Context::new)?;
        let result = context.get(symbol);
        match result {
            Err(CoreError::WordNotFound) => {
                if ctx != self.module.system_words {
                    self.module
                        .heap
                        .get_block(self.module.system_words)
                        .map(Context::new)
                        .and_then(|ctx| ctx.get(symbol))
                } else {
                    result
                }
            }
            _ => result,
        }
    }

    // fn find_word(&self, symbol: Symbol) -> Result<[Word; 2], CoreError> {
    //     let envs = self.envs.peek_all(0)?;

    //     for &addr in envs.iter().rev() {
    //         let context = self.module.heap.get_block(addr).map(Context::new)?;
    //         match context.get(symbol) {
    //             Ok(result) => return Ok(result),
    //             Err(CoreError::WordNotFound) => continue,
    //             Err(err) => return Err(err),
    //         }
    //     }

    //     Err(CoreError::WordNotFound)
    // }
}

impl<'a, T> Exec<'a, T>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    pub fn alloc<const N: usize>(&mut self, values: [Word; N]) -> Result<Offset, CoreError> {
        self.module.heap.alloc(values)
    }

    pub fn put_context(&mut self, symbol: Symbol, value: [Word; 2]) -> Result<(), CoreError> {
        let [ctx] = self.env.peek().ok_or(CoreError::InternalError)?;
        self.module
            .heap
            .get_block_mut(ctx)
            .map(Context::new)
            .and_then(|mut ctx| ctx.put(symbol, value))
    }

    pub fn new_context(&mut self, size: u32) -> Result<(), CoreError> {
        self.env.push([self.module.heap.alloc_context(size)?])
    }

    pub fn pop_context(&mut self) -> Result<Offset, CoreError> {
        self.env.pop().map(|[addr]| addr)
    }

    pub fn eval<I>(&mut self, words: I) -> Result<[Word; 2], CoreError>
    where
        I: IntoIterator<Item = Word>,
    {
        let mut iter = words.into_iter();

        while let Some(tag) = iter.next() {
            let value = match tag {
                Value::TAG_WORD => {
                    let symbol = iter.next().ok_or(CoreError::EndOfInput)?;
                    let result = self.find_word(symbol)?;
                    if result[0] == Value::TAG_STACK_VALUE {
                        if let Some([bp]) = self.base.peek() {
                            self.stack.get(bp + 2 + result[1] * 2)?
                        } else {
                            return Err(CoreError::InternalError);
                        }
                    } else {
                        result
                    }
                }
                _ => [tag, iter.next().ok_or(CoreError::EndOfInput)?],
            };

            let mut sp = self.stack.alloc(value)?;

            if let Some(arity) = match value[0] {
                Value::TAG_NATIVE_FN => Some(self.module.get_func(value[1])?.arity * 2), // remove * 2?
                Value::TAG_SET_WORD => Some(2),
                Value::TAG_FUNC => Some(self.module.get_array::<1>(value[1])?[0] * 2),
                _ => None,
            } {
                self.ops.push([sp, arity])?;
            }

            while let Some([bp, arity]) = self.ops.peek() {
                if sp == bp + arity {
                    self.ops.pop::<2>()?;
                    let [tag, value] = self.stack.get(bp)?;
                    let result = match tag {
                        Value::TAG_SET_WORD => {
                            let bp2 = self.stack.get(bp + 2)?;
                            self.put_context(value, bp2)?;
                            bp2
                        }
                        Value::TAG_NATIVE_FN => {
                            let native_fn = self.module.get_func(value)?;
                            (native_fn.func)(self, bp + 2)?
                        }
                        Value::TAG_FUNC => {
                            let [ctx, blk] = self.module.get_array(value + 1)?; // value -> [arity, ctx, blk]
                            self.env.push([ctx])?;
                            self.base.push([bp])?;
                            let block = self.module.get_block(blk)?;
                            let result = self.eval(block.iter().copied())?;
                            self.env.pop::<1>()?;
                            self.base.pop::<1>()?;
                            result
                        }
                        _ => {
                            return Err(CoreError::InternalError);
                        }
                    };
                    self.stack.set_len(bp)?;
                    sp = self.stack.alloc(result)?;
                } else {
                    break;
                }
            }
        }
        self.stack.pop::<2>().or(Ok([Value::TAG_NONE, 0]))
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
        let [bp] = self.ops.pop()?;
        let block_data = self.parse.pop_all(bp)?;
        let offset = self.module.heap.alloc_block(block_data)?;
        self.parse.push([Value::TAG_BLOCK, offset])
    }
}

//

// pub fn parse(module: &mut Module<&mut [Word]>, str: &str) -> Result<Box<[Word]>, CoreError> {
//     module.parse(str)
// }

pub fn eval(module: &mut Exec<&mut [Word]>, code: &[Word]) -> Result<[Word; 2], CoreError> {
    module.eval(code.iter().copied())
}

//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitespace_1() -> Result<(), CoreError> {
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse("  \t\n  ")?;
        let result = module.eval(&parsed)?;
        assert_eq!(Value::TAG_NONE, result[0]);
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
        assert_eq!([Value::TAG_INT, 5], result);
        Ok(())
    }

    #[test]
    fn test_add_1() -> Result<(), CoreError> {
        let input = "add 7 8";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!([Value::TAG_INT, 15], result);
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

    #[test]
    fn test_func_1() -> Result<(), CoreError> {
        let input = "f: func [a b] [add a b] f 1 2";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!([Value::TAG_INT, 3], result);
        Ok(())
    }

    #[test]
    fn test_func_2() -> Result<(), CoreError> {
        let input = "f: func [a b] [add a add b b] f 1 2";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!([Value::TAG_INT, 5], result);
        Ok(())
    }

    #[test]
    fn test_func_3() -> Result<(), CoreError> {
        let input = "f: func [n] [either lt n 2 [n] [add 1 f add n -1]] f 20";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!([Value::TAG_INT, 20], result);
        Ok(())
    }

    #[test]
    fn test_func_fib() -> Result<(), CoreError> {
        let input = "fib: func [n] [either lt n 2 [n] [add fib add n -1 fib add n -2]] fib 10";
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
        let parsed = module.parse(input)?;
        let result = module.eval(&parsed)?;
        assert_eq!([Value::TAG_INT, 55], result);
        Ok(())
    }
}

//
