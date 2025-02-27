// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::mem::{Context, Heap, Offset, Stack, Symbol, Word};
use crate::module::Module;
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
    #[error(transparent)]
    TryFromSliceError(#[from] std::array::TryFromSliceError),
    #[error("parse collector error")]
    ParseCollectorError,
    #[error("blob not found")]
    BlobNotFound,
}

// V A L U E

/// A value that can be used with the RebelDB VM
#[derive(Debug, Clone)]
pub enum Value {
    /// None/null value
    None,
    /// An integer value
    Int(i32),
    /// A string value - refers to a string stored in the heap
    String(Offset),
    /// A boolean value
    Bool(bool),
    /// A reference to a context
    Context(Offset),
    /// A reference to a block
    Block(Offset),
    /// A reference to a word
    Word(String),
    /// A reference to a native function
    NativeFn(Word),
    /// A reference to a function
    Func(Offset),
    /// A reference to a set word
    SetWord(Word),
    /// A stack value reference
    StackValue(Word),
}

/// Trait for values that can be converted into a Value
pub trait IntoValue {
    /// Convert this type to a Value
    fn into_value(self) -> Value;
}

// Implementation for common Rust types
impl IntoValue for i32 {
    fn into_value(self) -> Value {
        Value::Int(self)
    }
}

// String types can't be directly converted to Value::String anymore
// since they need to be allocated in the heap with Module::create_string.
// Instead, users need to use the Module API for string creation.
//
// We keep a stub implementation that panics with a helpful message for better error reporting.

impl IntoValue for &str {
    fn into_value(self) -> Value {
        panic!("Cannot directly convert &str to Value, use Module::create_string instead: '{}'", self)
    }
}

impl IntoValue for String {
    fn into_value(self) -> Value {
        panic!("Cannot directly convert String to Value, use Module::create_string instead: '{}'", self)
    }
}

impl IntoValue for bool {
    fn into_value(self) -> Value {
        Value::Bool(self)
    }
}

// Implementation for Offset - need to distinguish between different uses
impl IntoValue for Offset {
    fn into_value(self) -> Value {
        Value::Context(self)
    }
}

/// Container for block offsets to differentiate from context offsets
#[derive(Debug, Clone)]
pub struct BlockOffset(pub Offset);

impl IntoValue for BlockOffset {
    fn into_value(self) -> Value {
        Value::Block(self.0)
    }
}

impl IntoValue for &BlockOffset {
    fn into_value(self) -> Value {
        Value::Block(self.0)
    }
}

/// Container for word references
#[derive(Debug, Clone)]
pub struct WordRef(pub String);

impl IntoValue for WordRef {
    fn into_value(self) -> Value {
        Value::Word(self.0)
    }
}

impl IntoValue for &WordRef {
    fn into_value(self) -> Value {
        Value::Word(self.0.clone())
    }
}

// Special case for direct Value values
impl IntoValue for Value {
    fn into_value(self) -> Value {
        self
    }
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
    pub const TAG_STRING: Word = 11; // New tag for blob-based strings
    
    /// Convert a Value to its VM representation as a [tag, data] pair
    pub fn to_vm_value(&self, heap: &mut Heap<impl AsMut<[Word]> + AsRef<[Word]>>) -> Result<[Word; 2], CoreError> {
        match self {
            Value::None => Ok([Self::TAG_NONE, 0]),
            Value::Int(i) => Ok([Self::TAG_INT, *i as Word]),
            Value::String(offset) => {
                // String already has an offset to allocated VM representation
                // Now we need to get the [tag, data] pair from that offset
                let [tag, data] = heap.get(*offset).ok_or(CoreError::BoundsCheckFailed)?;
                Ok([tag, data])
            },
            Value::Bool(b) => Ok([Self::TAG_BOOL, if *b { 1 } else { 0 }]),
            Value::Context(c) => Ok([Self::TAG_CONTEXT, *c]),
            Value::Block(b) => Ok([Self::TAG_BLOCK, *b]),
            Value::Word(w) => {
                // Words must be inline strings (symbols can't be stored in blobs)
                let word_inline = inline_string(w).ok_or(CoreError::StringTooLong)?;
                let word_symbol = {
                    let mut sym_tbl = heap.get_symbols_mut().ok_or(CoreError::InternalError)?;
                    sym_tbl.get_or_insert(word_inline).ok_or(CoreError::SymbolTableFull)?
                };
                Ok([Self::TAG_WORD, word_symbol])
            },
            Value::NativeFn(n) => Ok([Self::TAG_NATIVE_FN, *n]),
            Value::Func(f) => Ok([Self::TAG_FUNC, *f]),
            Value::SetWord(s) => Ok([Self::TAG_SET_WORD, *s]),
            Value::StackValue(s) => Ok([Self::TAG_STACK_VALUE, *s]),
        }
    }
}

/// Create an inline string representation for short strings (up to 31 bytes)
/// The first byte stores the length, remaining 31 bytes store the string data
pub fn inline_string(string: &str) -> Option<[u32; 8]> {
    let bytes = string.as_bytes();
    let len = bytes.len();
    
    // Strings must be <= 31 bytes for inline representation
    if len <= 31 {
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

pub type NativeFn<T, B> = fn(module: &mut Exec<T, B>) -> Option<()>;

// Core module functions that are moved to module.rs

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

    fn next<T, B>(&mut self, module: &Module<T, B>) -> Option<[Word; 2]>
    where
        T: AsRef<[Word]>,
        B: crate::module::BlobStore,
    {
        let addr = self.ip;
        self.ip += 2;
        let offset = addr as usize;
        module
            .get_heap()
            .get_block(self.block)
            .and_then(|block| block.get(offset..offset + 2))
            .and_then(|value| value.try_into().ok())
    }
}

pub struct Exec<'a, T, B> {
    ip: IP,
    base_ptr: Offset,
    pub module: &'a mut Module<T, B>,
    stack: Stack<[Offset; 1024]>,
    arity: Stack<[Offset; 256]>,
    base: Stack<[Offset; 256]>,
    env: Stack<[Offset; 256]>,
    blocks: Stack<[Offset; 256]>,
}

impl<'a, T, B> Exec<'a, T, B> {
    pub fn new(module: &'a mut Module<T, B>) -> Option<Self> {
        let mut env = Stack::new([0; 256]);
        env.push([module.system_words()])?;
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

impl<T, B> Exec<'_, T, B>
where
    T: AsRef<[Word]>,
    B: crate::module::BlobStore,
{
    pub fn get_block<const N: usize>(&self, block: Offset, offset: Offset) -> Option<[Word; N]> {
        let offset = offset as usize;
        self.module
            .get_heap()
            .get_block(block)
            .and_then(|block| block.get(offset..offset + N))
            .and_then(|value| value.try_into().ok())
    }

    pub fn get_block_len(&self, block: Offset) -> Option<usize> {
        self.module.get_heap().get_block(block).map(|block| block.len())
    }

    fn find_word(&self, symbol: Symbol) -> Option<[Word; 2]> {
        let [ctx] = self.env.peek()?;
        let context = self.module.get_heap().get_block(ctx).map(Context::new)?;
        let result = context.get(symbol);
        match result {
            Err(CoreError::WordNotFound) => {
                if ctx != self.module.system_words() {
                    let system_words = self
                        .module
                        .get_heap()
                        .get_block(self.module.system_words())
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

impl<T, B> Exec<'_, T, B>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
    B: crate::module::BlobStore,
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
        self.module.get_heap_mut().alloc(values)
    }

    pub fn put_context(&mut self, symbol: Symbol, value: [Word; 2]) -> Option<()> {
        let [ctx] = self.env.peek()?;
        self.module
            .get_heap_mut()
            .get_block_mut(ctx)
            .map(Context::new)
            .and_then(|mut ctx| ctx.put(symbol, value))
    }

    pub fn new_context(&mut self, size: u32) -> Option<()> {
        self.env.push([self.module.get_heap_mut().alloc_context(size)?])
    }

    pub fn pop_context(&mut self) -> Option<Offset> {
        self.env.pop().map(|[addr]| addr)
    }

    fn get_value(&self, value: [Word; 2]) -> Option<[Word; 2]> {
        let [tag, word] = value;
        if tag == Value::TAG_WORD {
            let resolved = self.find_word(word);
            match resolved {
                Some([Value::TAG_STACK_VALUE, index]) => self
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
                Value::TAG_NATIVE_FN => {
                    Some((Op::CALL_NATIVE, self.module.get_func(value[1])?.arity))
                }
                Value::TAG_SET_WORD => Some((Op::SET_WORD, 1)),
                Value::TAG_FUNC => {
                    let addr = value[1];
                    let arity_value = self.module.get_heap().get::<1>(addr)?;
                    Some((Op::CALL_FUNC, arity_value[0]))
                },
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

    pub fn eval(&mut self) -> Option<[Word; 2]> {
        loop {
            if let Some(value) = self.next_value() {
                self.stack.alloc(value)?;
            } else {
                let stack_len = self.stack.len()?;
                match stack_len - self.base_ptr {
                    2 => {}
                    0 => {
                        self.stack.push([Value::TAG_NONE, 0])?;
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
                            // value is the func object pointer, it has [arity, ctx, blk]
                            // The actual memory layout is:
                            // At offset 'value': [arity]
                            // At offset 'value + 1': [ctx, blk]
                            // We don't need arity here, just get context and block
                            
                            // Get the context and block from the function object
                            let rest_entry = self.module.get_heap().get::<2>(value + 1)?;
                            let ctx = rest_entry[0];
                            let blk = rest_entry[1];
                            
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
                            self.stack.push([Value::TAG_CONTEXT, ctx])?;
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

        self.stack.pop::<2>().or(Some([Value::TAG_NONE, 0]))
    }
}

// P A R S E  C O L L E C T O R

// Moved out to parse.rs and adjusted to use the new module API

//

// pub fn parse(module: &mut Module<&mut [Word]>, str: &str) -> Result<Box<[Word]>, CoreError> {
//     module.parse(str)
// }

pub fn eval<B>(module: &mut Exec<&mut [Word], B>) -> Option<[Word; 2]> 
where 
    B: crate::module::BlobStore,
{
    module.eval()
}

//

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Module;

    use crate::MemoryBlobStore;
    
    fn eval(input: &str) -> Result<[Word; 2], CoreError> {
        let memory = vec![0; 0x10000].into_boxed_slice();
        let blob_store = MemoryBlobStore::new();
        let mut module = Module::init(memory, blob_store).expect("can't create module");
        let block = module.parse(input)?;
        module.eval(block).ok_or(CoreError::InternalError)
    }

    #[test]
    fn test_whitespace_1() -> Result<(), CoreError> {
        let result = eval("  \t\n  ")?;
        assert_eq!(Value::TAG_NONE, result[0]);
        Ok(())
    }

    #[test]
    fn test_string_1() -> Result<(), CoreError> {
        let result = eval(" \"hello\"  ")?;
        assert_eq!(Value::TAG_INLINE_STRING, result[0]);
        Ok(())
    }

    #[test]
    fn test_word_1() -> Result<(), CoreError> {
        let input = "42 \"world\" x: 5 x\n ";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 5], result);
        Ok(())
    }

    #[test]
    fn test_add_1() -> Result<(), CoreError> {
        let input = "add 7 8";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 15], result);
        Ok(())
    }

    #[test]
    fn test_add_2() -> Result<(), CoreError> {
        let input = "add 1 add 2 3";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 6], result[0..2]);
        Ok(())
    }

    #[test]
    fn test_add_3() -> Result<(), CoreError> {
        let input = "add add 3 4 5";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 12], result[0..2]);
        Ok(())
    }

    #[test]
    fn test_context_0() -> Result<(), CoreError> {
        let input = "context [x: 8]";
        let result = eval(input)?;
        assert_eq!(Value::TAG_CONTEXT, result[0]);
        Ok(())
    }

    #[test]
    fn test_func_1() -> Result<(), CoreError> {
        let input = "f: func [a b] [add a b] f 1 77";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 78], result);
        Ok(())
    }

    #[test]
    fn test_func_2() -> Result<(), CoreError> {
        let input = "f: func [a b] [add a add b b] f 1 2";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 5], result);
        Ok(())
    }

    #[test]
    fn test_either_1() -> Result<(), CoreError> {
        let input = "either lt 1 2 [1] [2]";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 1], result);
        Ok(())
    }

    #[test]
    fn test_either_2() -> Result<(), CoreError> {
        let input = "either lt 2 1 [1] [2]";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 2], result);
        Ok(())
    }

    #[test]
    fn test_do_1() -> Result<(), CoreError> {
        let input = "do [add 1 2]";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 3], result);
        Ok(())
    }

    #[test]
    fn test_func_3() -> Result<(), CoreError> {
        let input = "f: func [n] [either lt n 2 [n] [add 1 f add n -1]] f 20";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 20], result);
        Ok(())
    }

    #[test]
    fn test_func_fib() -> Result<(), CoreError> {
        let input = "fib: func [n] [either lt n 2 [n] [add fib add n -1 fib add n -2]] fib 10";
        let result = eval(input)?;
        assert_eq!([Value::TAG_INT, 55], result);
        Ok(())
    }
}

//
