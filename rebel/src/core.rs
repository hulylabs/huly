// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::boot::core_package;
use crate::mem::{Context, Heap, Offset, Stack, Symbol, SymbolTable, Word};
use crate::parse::{Collector, Parser, WordKind};
use crate::value::Value;
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
    #[error("word not found")]
    WordNotFound,
    #[error("stack underflow")]
    StackUnderflow,
    #[error("bad arguments")]
    BadArguments,
    #[error(transparent)]
    TryFromSliceError(#[from] std::array::TryFromSliceError),
    #[error(transparent)]
    ParserError(#[from] crate::parse::ParserError<MemoryError>),
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("out of memory")]
    OutOfMemory,
    #[error("unexpected error")]
    UnexpectedError,
}

// V A L U E

pub enum VmValue {
    None,
    Int(i32),
    String(Offset),
    Block(Offset),
    Context(Offset),
    Word(Symbol),
    SetWord(Symbol),
}

impl VmValue {
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

    pub fn vm_repr(&self) -> [Word; 2] {
        match self {
            VmValue::None => [Self::TAG_NONE, 0],
            VmValue::Int(value) => [Self::TAG_INT, *value as u32],
            VmValue::String(offset) => [Self::TAG_INLINE_STRING, *offset],
            VmValue::Word(symbol) => [Self::TAG_WORD, *symbol],
            VmValue::SetWord(symbol) => [Self::TAG_SET_WORD, *symbol],
            VmValue::Block(offset) => [Self::TAG_BLOCK, *offset],
            VmValue::Context(offset) => [Self::TAG_CONTEXT, *offset],
        }
    }
}

fn inline_string(string: &str) -> Option<[u32; 8]> {
    let bytes = string.as_bytes();
    let len = bytes.len();
    if len < 32 {
        let mut buf = [0; 8];
        buf[0] = len as u32;
        for (i, byte) in bytes.iter().enumerate() {
            let j = i + 1;
            buf[j / 4] |= (*byte as u32) << ((j % 4) * 8);
        }
        Some(buf)
    } else {
        None
    }
}

// M O D U L E

type NativeFn<T> = fn(module: &mut Exec<T>) -> Option<()>;

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

    fn get_func(&self, index: u32) -> Option<&FuncDesc<T>> {
        self.functions.get(index as usize)
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    pub fn init(data: T) -> Option<Self> {
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
        Some(module)
    }

    pub fn add_native_fn(&mut self, name: &str, func: NativeFn<T>, arity: u32) -> Option<()> {
        let index = self.functions.len() as u32;
        self.functions.push(FuncDesc { func, arity });
        let symbol = inline_string(name)?;
        let id = self.get_symbols_mut()?.get_or_insert(symbol)?;
        let mut words = self
            .heap
            .get_block_mut(self.system_words)
            .map(Context::new)?;
        words.put(id, [VmValue::TAG_NATIVE_FN, index])
    }

    pub fn eval(&mut self, block: Offset) -> Option<[Word; 2]> {
        let mut exec = Exec::new(self)?;
        exec.call(block)?;
        exec.eval()
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]>,
{
    fn get_array<const N: usize>(&self, addr: Offset) -> Option<[Word; N]> {
        self.heap.get(addr)
    }

    fn get_block<const N: usize>(&self, block: Offset, offset: Offset) -> Option<[Word; N]> {
        let offset = offset as usize;
        self.heap
            .get_block(block)
            .and_then(|block| block.get(offset..offset + N))
            .and_then(|value| value.try_into().ok())
    }
}

impl<T> Module<T>
where
    T: AsMut<[Word]>,
{
    fn get_symbols_mut(&mut self) -> Option<SymbolTable<&mut [Word]>> {
        let addr = self.heap.get_mut::<1>(Self::SYMBOLS).map(|[addr]| *addr)?;
        self.heap.get_block_mut(addr).map(SymbolTable::new)
    }

    pub fn parse(&mut self, code: &str) -> Result<Offset, CoreError> {
        let mut collector = ParseCollector::new(self);
        Parser::new(code, &mut collector).parse_block()?;
        let result = collector.parse.pop::<2>().ok_or(CoreError::InternalError)?;
        Ok(result[1])
    }

    pub fn alloc_string(&mut self, string: &str) -> Option<VmValue> {
        let transmuted = unsafe { std::mem::transmute::<&[u8], &[u32]>(string.as_bytes()) };
        self.heap
            .alloc_block(transmuted)
            .map(|addr| VmValue::String(addr))
    }

    pub fn get_or_insert_symbol(&mut self, symbol: &str) -> Option<Offset> {
        self.get_symbols_mut()?
            .get_or_insert(inline_string(symbol)?)
    }

    pub fn alloc_value(&mut self, value: Value) -> Option<VmValue> {
        None // TODO: implement
    }
}

// E X E C U T I O N  C O N T E X T

pub struct Op;

impl Op {
    const SET_WORD: u32 = 0;
    const CALL_NATIVE: u32 = 1;
    const CALL_FUNC: u32 = 2;
    const LEAVE: u32 = 3;
    pub const CONTEXT: u32 = 4;
}

#[derive(Debug)]
struct IP {
    block: Offset,
    ip: Offset,
}

impl IP {
    fn new(block: Offset, ip: Offset) -> Self {
        Self { block, ip }
    }

    fn next<T>(&mut self, module: &Module<T>) -> Option<[Word; 2]>
    where
        T: AsRef<[Word]>,
    {
        let addr = self.ip;
        self.ip += 2;
        module.get_block(self.block, addr)
    }
}

pub struct Exec<'a, T> {
    ip: IP,
    base_ptr: Offset,
    module: &'a mut Module<T>,
    stack: Stack<[Offset; 1024]>,
    arity: Stack<[Offset; 256]>,
    base: Stack<[Offset; 256]>,
    env: Stack<[Offset; 256]>,
    blocks: Stack<[Offset; 256]>,
}

impl<'a, T> Exec<'a, T> {
    fn new(module: &'a mut Module<T>) -> Option<Self> {
        let mut env = Stack::new([0; 256]);
        env.push([module.system_words])?;
        Some(Self {
            ip: IP::new(0, 0),
            base_ptr: 0,
            module,
            stack: Stack::new([0; 1024]),
            arity: Stack::new([0; 256]),
            base: Stack::new([0; 256]),
            blocks: Stack::new([0; 256]),
            env,
        })
    }
}

impl<'a, T> Exec<'a, T>
where
    T: AsRef<[Word]>,
{
    pub fn get_block<const N: usize>(&self, block: Offset, offset: Offset) -> Option<[Word; N]> {
        self.module.get_block(block, offset)
    }

    pub fn get_block_len(&self, block: Offset) -> Option<usize> {
        self.module.heap.get_block(block).map(|block| block.len())
    }

    fn find_word(&self, symbol: Symbol) -> Option<[Word; 2]> {
        let [ctx] = self.env.peek()?;
        let context = self.module.heap.get_block(ctx).map(Context::new)?;
        let result = context.get(symbol);
        match result {
            Err(CoreError::WordNotFound) => {
                if ctx != self.module.system_words {
                    let system_words = self
                        .module
                        .heap
                        .get_block(self.module.system_words)
                        .map(Context::new)?;
                    system_words.get(symbol).ok()
                } else {
                    result.ok()
                }
            }
            _ => result.ok(),
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
    pub fn pop<const N: usize>(&mut self) -> Option<[Word; N]> {
        self.stack.pop()
    }

    pub fn push<const N: usize>(&mut self, value: [Word; N]) -> Option<()> {
        self.stack.push(value)
    }

    pub fn call(&mut self, block: Offset) -> Option<()> {
        self.base_ptr = self.stack.len()?;
        let ret = [self.ip.block, self.ip.ip];
        self.ip = IP::new(block, 0);
        self.blocks.push(ret)
    }

    pub fn push_op(&mut self, op: Word, word: Word, arity: Word) -> Option<()> {
        self.arity.push([op, word, self.stack.len()?, arity])
    }

    pub fn alloc<const N: usize>(&mut self, values: [Word; N]) -> Option<Offset> {
        self.module.heap.alloc(values)
    }

    pub fn put_context(&mut self, symbol: Symbol, value: [Word; 2]) -> Option<()> {
        let [ctx] = self.env.peek()?;
        self.module
            .heap
            .get_block_mut(ctx)
            .map(Context::new)
            .and_then(|mut ctx| ctx.put(symbol, value))
    }

    pub fn new_context(&mut self, size: u32) -> Option<()> {
        self.env.push([self.module.heap.alloc_context(size)?])
    }

    pub fn pop_context(&mut self) -> Option<Offset> {
        self.env.pop().map(|[addr]| addr)
    }

    fn get_value(&self, value: [Word; 2]) -> Option<[Word; 2]> {
        let [tag, word] = value;
        if tag == VmValue::TAG_WORD {
            let resolved = self.find_word(word);
            match resolved {
                Some([VmValue::TAG_STACK_VALUE, index]) => self
                    .base
                    .peek()
                    .and_then(|[bp]| self.stack.get(bp + index * 2)),
                _ => resolved,
            }
        } else {
            Some(value)
        }
    }

    fn next_value(&mut self) -> Option<[Word; 2]> {
        while let Some(cmd) = self.ip.next(self.module) {
            let value = self.get_value(cmd)?;

            if let Some((op, arity)) = match value[0] {
                VmValue::TAG_NATIVE_FN => {
                    Some((Op::CALL_NATIVE, self.module.get_func(value[1])?.arity))
                }
                VmValue::TAG_SET_WORD => Some((Op::SET_WORD, 1)),
                VmValue::TAG_FUNC => {
                    Some((Op::CALL_FUNC, self.module.get_array::<1>(value[1])?[0]))
                }
                _ => None,
            } {
                let sp = self.stack.len()?;
                self.arity.push([op, value[1], sp, arity * 2])?;
            } else {
                return Some(value);
            }
        }
        None
    }

    fn eval(&mut self) -> Option<[Word; 2]> {
        loop {
            if let Some(value) = self.next_value() {
                self.stack.alloc(value)?;
            } else {
                let stack_len = self.stack.len()?;
                match stack_len - self.base_ptr {
                    2 => {}
                    0 => {
                        self.stack.push([VmValue::TAG_NONE, 0])?;
                    }
                    _ => {
                        let result = self.stack.pop::<2>()?;
                        self.stack.set_len(self.base_ptr)?;
                        self.stack.push(result)?;
                    }
                }
                let [block, ip] = self.blocks.pop()?;
                if block != 0 {
                    self.ip = IP::new(block, ip);
                } else {
                    break;
                }
            }

            while let Some([bp, arity]) = self.arity.peek() {
                let sp = self.stack.len()?;
                if sp == bp + arity {
                    let [op, value, _, _] = self.arity.pop()?;
                    match op {
                        Op::SET_WORD => {
                            let result = self.stack.pop()?;
                            self.put_context(value, result)?;
                        }
                        Op::CALL_NATIVE => {
                            let native_fn = self.module.get_func(value)?;
                            (native_fn.func)(self)?;
                        }
                        Op::CALL_FUNC => {
                            let [ctx, blk] = self.module.get_array(value + 1)?; // value -> [arity, ctx, blk]
                            self.env.push([ctx])?;
                            self.base.push([bp])?;
                            self.arity.push([Op::LEAVE, 0, sp, 2])?;
                            self.call(blk)?;
                            break;
                        }
                        Op::LEAVE => {
                            self.env.pop::<1>()?;
                            let [stack] = self.base.pop::<1>()?;
                            let result = self.stack.pop::<2>()?;
                            self.stack.set_len(stack)?;
                            self.stack.push(result)?;
                            self.base_ptr = stack;
                        }
                        Op::CONTEXT => {
                            let ctx = self.pop_context()?;
                            self.stack.push([VmValue::TAG_CONTEXT, ctx])?;
                        }
                        _ => {
                            return None;
                        }
                    };
                } else {
                    break;
                }
            }
        }

        self.stack.pop::<2>().or(Some([VmValue::TAG_NONE, 0]))
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
    type Error = MemoryError;

    fn string(&mut self, string: &str) -> Result<(), Self::Error> {
        self.module
            .alloc_string(string)
            .and_then(|value| self.parse.push(value.vm_repr()))
            .ok_or(MemoryError::OutOfMemory)
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Result<(), Self::Error> {
        self.module
            .get_or_insert_symbol(word)
            .and_then(|id| {
                let value = match kind {
                    WordKind::Word => VmValue::Word(id),
                    WordKind::SetWord => VmValue::SetWord(id),
                };
                self.parse.push(value.vm_repr())
            })
            .ok_or(MemoryError::OutOfMemory)
    }

    fn integer(&mut self, value: i32) -> Result<(), MemoryError> {
        self.parse
            .push([VmValue::TAG_INT, value as u32])
            .ok_or(MemoryError::OutOfMemory)
    }

    fn begin_block(&mut self) -> Result<(), MemoryError> {
        self.parse
            .len()
            .and_then(|len| self.ops.push([len]))
            .ok_or(MemoryError::OutOfMemory)
    }

    fn end_block(&mut self) -> Result<(), MemoryError> {
        let [bp] = self.ops.pop().ok_or(MemoryError::UnexpectedError)?;
        let block_data = self.parse.pop_all(bp).ok_or(MemoryError::UnexpectedError)?;
        let offset = self
            .module
            .heap
            .alloc_block(block_data)
            .ok_or(MemoryError::OutOfMemory)?;
        self.parse
            .push([VmValue::TAG_BLOCK, offset])
            .ok_or(MemoryError::OutOfMemory)
    }
}

//

// pub fn parse(module: &mut Module<&mut [Word]>, str: &str) -> Result<Box<[Word]>, CoreError> {
//     module.parse(str)
// }

pub fn eval(module: &mut Exec<&mut [Word]>) -> Option<[Word; 2]> {
    module.eval()
}

//

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(input: &str) -> Result<[Word; 2], CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");
        let block = module.parse(input)?;
        module.eval(block).ok_or(CoreError::InternalError)
    }

    #[test]
    fn test_whitespace_1() -> Result<(), CoreError> {
        let result = eval("  \t\n  ")?;
        assert_eq!(VmValue::TAG_NONE, result[0]);
        Ok(())
    }

    #[test]
    fn test_string_1() -> Result<(), CoreError> {
        let result = eval(" \"hello\"  ")?;
        assert_eq!(VmValue::TAG_INLINE_STRING, result[0]);
        Ok(())
    }

    #[test]
    fn test_word_1() -> Result<(), CoreError> {
        let input = "42 \"world\" x: 5 x\n ";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 5], result);
        Ok(())
    }

    #[test]
    fn test_add_1() -> Result<(), CoreError> {
        let input = "add 7 8";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 15], result);
        Ok(())
    }

    #[test]
    fn test_add_2() -> Result<(), CoreError> {
        let input = "add 1 add 2 3";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 6], result[0..2]);
        Ok(())
    }

    #[test]
    fn test_add_3() -> Result<(), CoreError> {
        let input = "add add 3 4 5";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 12], result[0..2]);
        Ok(())
    }

    #[test]
    fn test_context_0() -> Result<(), CoreError> {
        let input = "context [x: 8]";
        let result = eval(input)?;
        assert_eq!(VmValue::TAG_CONTEXT, result[0]);
        Ok(())
    }

    #[test]
    fn test_func_1() -> Result<(), CoreError> {
        let input = "f: func [a b] [add a b] f 1 77";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 78], result);
        Ok(())
    }

    #[test]
    fn test_func_2() -> Result<(), CoreError> {
        let input = "f: func [a b] [add a add b b] f 1 2";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 5], result);
        Ok(())
    }

    #[test]
    fn test_either_1() -> Result<(), CoreError> {
        let input = "either lt 1 2 [1] [2]";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 1], result);
        Ok(())
    }

    #[test]
    fn test_either_2() -> Result<(), CoreError> {
        let input = "either lt 2 1 [1] [2]";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 2], result);
        Ok(())
    }

    #[test]
    fn test_do_1() -> Result<(), CoreError> {
        let input = "do [add 1 2]";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 3], result);
        Ok(())
    }

    #[test]
    fn test_func_3() -> Result<(), CoreError> {
        let input = "f: func [n] [either lt n 2 [n] [add 1 f add n -1]] f 20";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 20], result);
        Ok(())
    }

    #[test]
    fn test_func_fib() -> Result<(), CoreError> {
        let input = "fib: func [n] [either lt n 2 [n] [add fib add n -1 fib add n -2]] fib 10";
        let result = eval(input)?;
        assert_eq!([VmValue::TAG_INT, 55], result);
        Ok(())
    }
}

//
