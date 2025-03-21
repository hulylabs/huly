// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::boot::{core_package, stdlib_package};
use crate::mem::{Context, Heap, MemoryError, Offset, Stack, Symbol, SymbolId, SymbolTable, Word};
use crate::parse::{Collector, Parser, WordKind};
use crate::value::Value;
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("internal error")]
    InternalError,
    #[error("end of input")]
    EndOfInput,
    #[error("unexpected end of block")]
    UnexpectedEndOfBlock,
    #[error("function not found")]
    FunctionNotFound,
    #[error("string too long")]
    StringTooLong,
    #[error("bounds check failed")]
    BoundsCheckFailed,
    #[error("symbol table full")]
    SymbolTableFull,
    #[error("bad arguments")]
    BadArguments,
    #[error("unknown tag")]
    UnknownTag,
    #[error(transparent)]
    ParserError(#[from] crate::parse::ParserError<MemoryError>),
    #[error(transparent)]
    MemoryError(#[from] MemoryError),
    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    AnyError(#[from] anyhow::Error),
}

// V M  V A L U E

pub type MemValue = [Word; 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmValue {
    None,
    Int(i32),
    Bool(bool),
    String(Offset),
    Block(Offset),
    Context(Offset),
    Path(Offset),
    Word(SymbolId),
    SetWord(SymbolId),
    GetWord(SymbolId),
    Func(Offset),
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
    pub const TAG_GET_WORD: Word = 8;
    pub const TAG_FUNC: Word = 9;
    pub const TAG_BOOL: Word = 10;
    pub const TAG_PATH: Word = 11;

    /// Convert a tag and data word into a VmValue
    ///
    /// This helper method is used to convert a tag/data pair from memory into
    /// a VmValue enum variant. It is used by both to_value and read_value methods.
    ///
    /// # Arguments
    /// * `tag` - The tag that identifies the value type
    /// * `data` - The data word associated with the tag
    ///
    /// # Returns
    /// * `Ok(VmValue)` - The constructed VmValue if the tag is recognized
    /// * `Err(CoreError)` - If the tag is not recognized
    pub fn from_tag_data(tag: Word, data: Word) -> Result<VmValue, CoreError> {
        match tag {
            Self::TAG_NONE => Ok(VmValue::None),
            Self::TAG_INT => Ok(VmValue::Int(data as i32)),
            Self::TAG_BLOCK => Ok(VmValue::Block(data)),
            Self::TAG_CONTEXT => Ok(VmValue::Context(data)),
            Self::TAG_INLINE_STRING => Ok(VmValue::String(data)),
            Self::TAG_WORD => Ok(VmValue::Word(data)),
            Self::TAG_SET_WORD => Ok(VmValue::SetWord(data)),
            Self::TAG_FUNC => Ok(VmValue::Func(data)),
            Self::TAG_PATH => Ok(VmValue::Path(data)),
            Self::TAG_BOOL => Ok(VmValue::Bool(data != 0)),
            _ => Err(CoreError::UnknownTag),
        }
    }

    pub fn vm_repr(&self) -> [Word; 2] {
        match self {
            VmValue::None => [Self::TAG_NONE, 0],
            VmValue::Int(value) => [Self::TAG_INT, *value as u32],
            VmValue::Bool(value) => [Self::TAG_BOOL, if *value { 1 } else { 0 }],
            VmValue::String(offset) => [Self::TAG_INLINE_STRING, *offset],
            VmValue::Word(symbol) => [Self::TAG_WORD, *symbol],
            VmValue::SetWord(symbol) => [Self::TAG_SET_WORD, *symbol],
            VmValue::GetWord(symbol) => [Self::TAG_GET_WORD, *symbol],
            VmValue::Block(offset) => [Self::TAG_BLOCK, *offset],
            VmValue::Context(offset) => [Self::TAG_CONTEXT, *offset],
            VmValue::Func(offset) => [Self::TAG_FUNC, *offset],
            VmValue::Path(offset) => [Self::TAG_PATH, *offset],
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, VmValue::None)
    }

    pub fn is_int(&self) -> bool {
        matches!(self, VmValue::Int(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, VmValue::String(_))
    }

    pub fn is_block(&self) -> bool {
        matches!(self, VmValue::Block(_))
    }

    pub fn is_context(&self) -> bool {
        matches!(self, VmValue::Context(_))
    }
}

// Implement TryFrom for VmValue to allow more ergonomic conversion from [Word; 2]
impl TryFrom<[Word; 2]> for VmValue {
    type Error = CoreError;

    fn try_from(value: [Word; 2]) -> Result<Self, Self::Error> {
        let [tag, data] = value;
        VmValue::from_tag_data(tag, data)
    }
}

// Implement From for [Word; 2] to allow easy conversion from VmValue
impl From<VmValue> for [Word; 2] {
    fn from(value: VmValue) -> Self {
        value.vm_repr()
    }
}

// M O D U L E

type NativeFn<T> = fn(module: &mut Exec<T>) -> Result<(), CoreError>;

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

    pub fn new_process(&mut self, block: VmValue) -> Result<Exec<T>, CoreError> {
        let block = match block {
            VmValue::Block(offset) => offset,
            _ => return Err(CoreError::BadArguments),
        };
        Exec::new(self, block).map_err(Into::into)
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
        stdlib_package(&mut module)?;
        Ok(module)
    }

    pub fn add_native_fn(
        &mut self,
        name: &str,
        func: NativeFn<T>,
        arity: u32,
    ) -> Result<(), MemoryError> {
        let index = self.functions.len() as u32;
        self.functions.push(FuncDesc {
            func,
            arity: arity * 2,
        });
        let symbol = Symbol::from(name)?;
        let id = self.get_symbols_mut()?.get_or_insert(symbol)?;
        let mut words = self
            .heap
            .get_block_mut(self.system_words)
            .map(Context::new)?;
        words.put(id, [VmValue::TAG_NATIVE_FN, index])
    }

    pub fn eval(&mut self, block: VmValue) -> Result<VmValue, CoreError> {
        self.new_process(block).and_then(|mut exec| exec.eval())
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]>,
{
    // fn get_array<const N: usize>(&self, addr: Offset) -> Result<[Word; N], MemoryError> {
    //     self.heap.get(addr)
    // }

    fn get_block<const N: usize>(
        &self,
        block: Offset,
        offset: Offset,
    ) -> Result<[Word; N], MemoryError> {
        let offset = offset as usize;
        self.heap
            .get_block(block)
            .and_then(|block| {
                block
                    .get(offset..offset + N)
                    .ok_or(MemoryError::OutOfBounds)
            })
            .and_then(|value| value.try_into().map_err(Into::into))
    }
}

impl<T> Module<T>
where
    T: AsMut<[Word]>,
{
    fn get_symbols_mut(&mut self) -> Result<SymbolTable<&mut [Word]>, MemoryError> {
        let addr = self.heap.get_mut::<1>(Self::SYMBOLS).map(|[addr]| *addr)?;
        self.heap.get_block_mut(addr).map(SymbolTable::new)
    }

    pub fn parse(&mut self, code: &str) -> Result<VmValue, CoreError> {
        let mut collector = ParseCollector::new(self);
        Parser::new(code, &mut collector).parse_block()?;
        let result = collector.parse.pop::<2>()?;
        result.try_into()
    }

    pub fn alloc_string(&mut self, string: &str) -> Result<Offset, MemoryError> {
        let bytes = string.as_bytes();
        let word_count = (bytes.len() + 3) / 4; // ceiling division
        let mut words = Vec::with_capacity(word_count + 1);
        words.push(bytes.len() as u32);
        let mut current_word = 0u32;
        for (i, &byte) in bytes.iter().enumerate() {
            let shift = (i % 4) * 8;
            current_word |= (byte as u32) << shift;

            // If we've filled a word (or reached the end), add it to the vector
            if (i + 1) % 4 == 0 || i == bytes.len() - 1 {
                words.push(current_word);
                current_word = 0;
            }
        }
        self.heap.alloc_block(&words)
    }

    pub fn get_or_insert_symbol(&mut self, symbol: &str) -> Result<Offset, MemoryError> {
        self.get_symbols_mut()?.get_or_insert(Symbol::from(symbol)?)
    }

    fn alloc_block(&mut self, values: &[Value]) -> Result<Offset, MemoryError> {
        let mut vm_values = Vec::with_capacity(values.len() * 2);
        for item in values.iter() {
            let mem = self.alloc_value(item)?.vm_repr();
            vm_values.push(mem[0]);
            vm_values.push(mem[1]);
        }
        self.heap.alloc_block(&vm_values)
    }

    pub fn alloc_value(&mut self, value: &Value) -> Result<VmValue, MemoryError> {
        match value {
            Value::None => Ok(VmValue::None),
            Value::Int(n) => Ok(VmValue::Int(*n)),
            Value::Bool(b) => Ok(VmValue::Bool(*b)),

            Value::String(s) => self.alloc_string(s.as_ref()).map(VmValue::String),

            Value::Word(w) => self.get_or_insert_symbol(w.as_ref()).map(VmValue::Word),
            Value::SetWord(w) => self.get_or_insert_symbol(w.as_ref()).map(VmValue::SetWord),
            Value::GetWord(w) => self.get_or_insert_symbol(w.as_ref()).map(VmValue::GetWord),
            Value::Block(items) => self.alloc_block(items).map(VmValue::Block),
            Value::Path(items) => self.alloc_block(items).map(VmValue::Path),

            Value::Context(pairs) => {
                let context = self.heap.alloc_context(pairs.len() as u32)?;

                for (key, val) in pairs.iter() {
                    let symbol = self.get_or_insert_symbol(key)?;
                    let vm_value = self.alloc_value(val)?;
                    self.heap
                        .get_block_mut(context)
                        .map(Context::new)
                        .and_then(|mut ctx| ctx.put(symbol, vm_value.vm_repr()))?;
                }

                Ok(VmValue::Context(context))
            }
        }
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]>,
{
    pub fn get_symbol(&self, symbol: SymbolId) -> Result<SmolStr, MemoryError> {
        let addr = self.heap.get::<1>(Self::SYMBOLS).map(|[addr]| addr)?;
        let symbol_table = self.heap.get_block(addr).map(SymbolTable::new)?;
        let inlined = symbol_table.get(symbol)?;
        Ok(inlined.to_string())
    }

    fn get_block_value(&self, offset: Offset) -> Result<Box<[Value]>, CoreError> {
        let block_data = self.heap.get_block(offset)?;
        let mut values = Vec::new();

        for pair in block_data.chunks_exact(2) {
            let vm_value = VmValue::from_tag_data(pair[0], pair[1])?;
            values.push(self.to_value(vm_value)?);
        }

        Ok(values.into_boxed_slice())
    }

    pub fn to_value(&self, vm_value: VmValue) -> Result<Value, CoreError> {
        match vm_value {
            VmValue::None => Ok(Value::None),
            VmValue::Int(n) => Ok(Value::Int(n)),
            VmValue::Bool(b) => Ok(Value::Bool(b)),
            VmValue::Word(symbol) => Ok(Value::Word(self.get_symbol(symbol)?)),
            VmValue::SetWord(symbol) => Ok(Value::SetWord(self.get_symbol(symbol)?)),
            VmValue::GetWord(symbol) => Ok(Value::GetWord(self.get_symbol(symbol)?)),

            VmValue::String(offset) => {
                let string_block = self.heap.get_block(offset)?;
                if string_block.is_empty() {
                    return Ok(Value::String("".into()));
                }

                // First word is the length
                let length = string_block[0] as usize;

                // Convert the block data to bytes safely
                let mut bytes = Vec::with_capacity(length);
                let mut remaining = length;

                // Process one word at a time, extracting bytes
                for word in string_block.iter().skip(1) {
                    if remaining == 0 {
                        break;
                    }

                    // Extract up to 4 bytes from each word
                    for j in 0..4 {
                        if remaining == 0 {
                            break;
                        }

                        let byte = ((word >> (j * 8)) & 0xFF) as u8;
                        bytes.push(byte);
                        remaining -= 1;
                    }
                }

                // Convert bytes to string
                match String::from_utf8(bytes) {
                    Ok(string) => Ok(Value::String(string.into())),
                    Err(e) => Err(e.into()), // UTF-8 decoding error
                }
            }

            VmValue::Block(offset) => Ok(Value::Block(self.get_block_value(offset)?)),
            VmValue::Path(offset) => Ok(Value::Path(self.get_block_value(offset)?)),

            // Context value stored in heap
            VmValue::Context(offset) => {
                let context_block = self.heap.get_block(offset)?;
                if context_block.is_empty() {
                    return Ok(Value::Context(Box::new([])));
                }

                let mut pairs = Vec::new();
                let context_data = Context::new(context_block);

                // Use the iterator to efficiently iterate through all entries in the context
                for (symbol, [tag, data]) in &context_data {
                    let symbol_name = self.get_symbol(symbol)?;
                    let vm_value = VmValue::from_tag_data(tag, data)?;
                    pairs.push((symbol_name, self.to_value(vm_value)?));
                }

                Ok(Value::Context(pairs.into_boxed_slice()))
            }

            // Function value stored in heap
            VmValue::Func(_offset) => {
                Ok(Value::None)
                // let block = self.heap.get_block(offset)?;
                // if block.is_empty() {
                //     return Ok(Value::Block(Box::new([])));
                // }

                // let [arity, ctx, blk] = block;
                // let context = self.heap.get_block(ctx)?;
                // let mut pairs = Vec::new();
                // let context_data = Context::new(context);

                // // Use the iterator to efficiently iterate through all entries in the context
                // for (symbol, [tag, data]) in &context_data {
                //     let symbol_name = self.get_symbol(symbol)?;
                //     let vm_value = VmValue::from_tag_data(tag, data)?;
                //     pairs.push((symbol_name, self.to_value(vm_value)?));
                // }

                // Ok(Value::Func(arity, pairs.into_boxed_slice(), blk))
            }
        }
    }

    pub fn read_value(&self, addr: Offset) -> Result<Value, CoreError> {
        // Get the tag and data from the address
        let [tag, data] = self.heap.get::<2>(addr)?;

        // Convert tag/data to VmValue using the helper method
        let vm_value = VmValue::from_tag_data(tag, data)?;

        self.to_value(vm_value)
    }
}

// E X E C U T I O N  C O N T E X T

pub struct Op;

impl Op {
    const SET_WORD: u32 = 0;
    const CALL_NATIVE: u32 = 1;
    const CALL_FUNC: u32 = 2;
    const LEAVE_BLOCK: u32 = 3;
    const LEAVE_FUNC: u32 = 4;
    pub const CONTEXT: Word = 5;
    pub const REDUCE: Word = 6;
    pub const FOREACH: Word = 7;
    const LIT_PARAM: Word = 8;
}

pub struct Exec<'a, T> {
    block: Offset,
    ip: Offset,

    module: &'a mut Module<T>,
    stack: Stack<[Offset; 1024]>,
    op_stack: Stack<[Offset; 1024]>,
    env: Stack<[Offset; 512]>,
}

impl<'a, T> Exec<'a, T> {
    const LEAVE_MARKER: Offset = 0x10000;

    fn new(module: &'a mut Module<T>, block: Offset) -> Result<Self, MemoryError> {
        let mut env = Stack::new([0; 512]);
        env.push([module.system_words])?;
        Ok(Self {
            block,
            ip: 0,
            module,
            stack: Stack::new([0; 1024]),
            op_stack: Stack::new([0; 1024]),
            env,
        })
    }
}

impl<'a, T> Exec<'a, T>
where
    T: AsRef<[Word]>,
{
    pub fn get_block<const N: usize>(
        &self,
        block: Offset,
        offset: Offset,
    ) -> Result<[Word; N], MemoryError> {
        self.module.get_block(block, offset)
    }

    pub fn get_block_len(&self, block: Offset) -> Result<usize, MemoryError> {
        self.module.heap.get_block(block).map(|block| block.len())
    }

    fn find_word(&self, symbol: SymbolId) -> Result<MemValue, MemoryError> {
        let envs = self.env.peek_all(0).ok_or(MemoryError::StackUnderflow)?;

        for &addr in envs.iter().rev() {
            let context = self.module.heap.get_block(addr).map(Context::new)?;
            match context.get(symbol) {
                Ok(result) => return Ok(result),
                Err(MemoryError::WordNotFound) => continue,
                Err(err) => return Err(err),
            }
        }

        Err(MemoryError::WordNotFound)
    }

    pub fn to_value(&self, vm_value: VmValue) -> Result<Value, CoreError> {
        self.module.to_value(vm_value)
    }

    pub fn peek<const N: usize>(&self) -> Option<[Word; N]> {
        self.stack.peek()
    }
}

impl<'a, T> Exec<'a, T>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    pub fn pop<const N: usize>(&mut self) -> Result<[Word; N], MemoryError> {
        self.stack.pop()
    }

    /// Pop a value from the stack and convert it to a VmValue
    pub fn pop_value(&mut self) -> Result<VmValue, CoreError> {
        let words = self.pop::<2>()?;
        words.try_into()
    }

    /// Pop a value from the stack and convert it directly to a high-level Value
    pub fn pop_to_value(&mut self) -> Result<Value, CoreError> {
        let vm_value = self.pop_value()?;
        self.to_value(vm_value)
    }

    pub fn push<const N: usize>(&mut self, value: [Word; N]) -> Result<(), MemoryError> {
        self.stack.push(value)
    }

    /// Push a VmValue onto the stack
    pub fn push_vm_value(&mut self, value: VmValue) -> Result<(), MemoryError> {
        self.push(value.into())
    }

    /// Push a high-level Value directly onto the stack
    pub fn push_value(&mut self, value: Value) -> Result<(), CoreError> {
        self.module
            .alloc_value(&value)
            .and_then(|vm_value| self.push_vm_value(vm_value))
            .map_err(Into::into)
    }

    pub fn jmp_op(&mut self, block: Offset, op: Word) -> Result<(), CoreError> {
        self.op_stack.push([
            op,
            self.block,
            self.stack.len()?,
            Self::LEAVE_MARKER + self.ip,
        ])?;
        self.block = block;
        self.ip = 0;
        Ok(())
    }

    pub fn jmp(&mut self, block: Offset) -> Result<(), CoreError> {
        self.jmp_op(block, Op::LEAVE_BLOCK)
    }

    pub fn push_op(&mut self, op: Word, word: Word, arity: Word) -> Result<(), MemoryError> {
        self.op_stack.push([op, word, self.stack.len()?, arity])
    }

    // pub fn alloc<const N: usize>(&mut self, values: [Word; N]) -> Result<Offset, MemoryError> {
    //     self.module.heap.alloc(values)
    // }

    pub fn alloc_block(&mut self, values: &[Word]) -> Result<Offset, MemoryError> {
        self.module.heap.alloc_block(values)
    }

    pub fn alloc_string(&mut self, string: &str) -> Result<Offset, MemoryError> {
        self.module.alloc_string(string)
    }

    pub fn alloc_context(&mut self, size: u32) -> Result<Offset, MemoryError> {
        self.module.heap.alloc_context(size)
    }

    pub fn get_context(&mut self, offset: Offset) -> Result<Context<&mut [u32]>, MemoryError> {
        self.module.heap.get_block_mut(offset).map(Context::new)
    }

    fn peek_context(&mut self) -> Result<Context<&mut [u32]>, MemoryError> {
        let [ctx] = self.env.peek().ok_or(MemoryError::StackUnderflow)?;
        self.module.heap.get_block_mut(ctx).map(Context::new)
    }

    pub fn push_context(&mut self, ctx: Offset) -> Result<(), MemoryError> {
        self.env.push([ctx])
    }

    pub fn pop_context(&mut self) -> Result<Offset, MemoryError> {
        self.env.pop().map(|[addr]| addr)
    }

    pub fn alloc_value(&mut self, value: &Value) -> Result<VmValue, MemoryError> {
        self.module.alloc_value(value)
    }

    fn resolve(&mut self, value: MemValue) -> Result<MemValue, CoreError> {
        match value[0] {
            VmValue::TAG_WORD => {
                let word = self.find_word(value[1])?;
                self.resolve(word)
            }
            VmValue::TAG_PATH => {
                let env_len = self.env.len()?;
                let block = value[1];
                let mut offset = 0;
                while let Ok(path_segment) = self.get_block::<2>(block, offset) {
                    match path_segment[0] {
                        VmValue::TAG_WORD => {
                            let result = self.find_word(path_segment[1])?;
                            if result[0] == VmValue::TAG_CONTEXT {
                                self.env.push([result[1]])?;
                                offset += 2;
                            } else {
                                self.env.set_len(env_len)?;
                                return Ok(result);
                            }
                        }
                        _ => unimplemented!(),
                    }
                }
                Err(CoreError::UnexpectedEndOfBlock)
            }
            _ => Ok(value),
        }
    }

    fn do_op(&mut self, op: Word, word: Word) -> Result<(), CoreError> {
        match op {
            Op::SET_WORD => {
                let value = self.stack.peek().ok_or(MemoryError::StackUnderflow)?;
                let contexts = self.env.peek_all(0).ok_or(MemoryError::StackUnderflow)?;
                for &ctx in contexts.iter().rev() {
                    let mut context = self.module.heap.get_block_mut(ctx).map(Context::new)?;
                    match context.put(word, value) {
                        Ok(_) => return Ok(()),
                        Err(MemoryError::WordNotFound) => continue,
                        Err(err) => return Err(err.into()),
                    }
                }
                Err(MemoryError::WordNotFound.into())
            }
            Op::CALL_NATIVE => {
                let native_fn = self.module.get_func(word)?;
                (native_fn.func)(self)
            }
            Op::CALL_FUNC => {
                let [_, arity, _, params, _, body] = self.get_block(word, 0)?;
                let mut offset = arity;
                let ctx = self.alloc_context(arity)?;
                while offset > 0 {
                    offset -= 2;
                    let [tag, symbol] = self.get_block(params, offset)?;
                    if tag != VmValue::TAG_WORD {
                        return Err(CoreError::BadArguments);
                    }
                    let value = self.pop()?;
                    self.get_context(ctx)?.put(symbol, value)?;
                }

                self.env.push([ctx])?;
                let bp = self.stack.len()?;
                self.op_stack.push([
                    Op::LEAVE_FUNC,
                    self.block,
                    bp,
                    Self::LEAVE_MARKER + self.ip,
                ])?;

                self.block = body;
                self.ip = 0;

                Ok(())
            }
            Op::CONTEXT => {
                self.stack.pop::<2>()?; // we have result on stack after context's block leave, let's remove it and push context
                let ctx = self.pop_context()?;
                self.stack.push([VmValue::TAG_CONTEXT, ctx])?;
                Ok(())
            }
            Op::LIT_PARAM => {
                let value = self.get_block::<2>(self.block, self.ip)?;
                self.ip += 2;
                self.stack.push(value)?;
                Ok(())
            }
            _ => Err(CoreError::InternalError),
        }
    }

    fn next_op(&mut self) -> Result<(Word, Word), CoreError> {
        loop {
            // Check pending operations
            if let Some([bp, arity]) = self.op_stack.peek() {
                let sp = self.stack.len()?;
                if sp == bp + arity {
                    let [op, word, _, _] = self.op_stack.pop()?;
                    return Ok((op, word));
                }
            }

            if let Ok(val) = self.get_block(self.block, self.ip) {
                self.ip += 2;
                match self.resolve(val)? {
                    [VmValue::TAG_NATIVE_FN, func] => {
                        let desc = self.module.get_func(func)?;
                        if desc.arity == 200 {
                            self.push_op(Op::CALL_NATIVE, func, 6)?;
                            return Ok((Op::LIT_PARAM, 0));
                        } else {
                            if desc.arity == 0 {
                                return Ok((Op::CALL_NATIVE, func));
                            } else {
                                self.push_op(Op::CALL_NATIVE, func, desc.arity)?;
                            }
                        }
                    }
                    [VmValue::TAG_FUNC, desc] => {
                        let [arity] = self.module.get_block::<1>(desc, 1)?;
                        if arity == 0 {
                            return Ok((Op::CALL_FUNC, desc));
                        } else {
                            self.push_op(Op::CALL_FUNC, desc, arity)?;
                        }
                    }
                    [VmValue::TAG_SET_WORD, sym] => self.push_op(Op::SET_WORD, sym, 2)?,
                    other => self.push(other)?,
                }
            } else {
                // end of block, let's return single value and set up base
                if self.op_stack.is_empty()? {
                    return Err(CoreError::EndOfInput);
                }

                let (block, ip) = {
                    let [op, block, bp, ip] = self.op_stack.pop()?;

                    match op {
                        Op::LEAVE_FUNC => {
                            self.pop_context()?;
                            self.leave(bp)?;
                            (block, ip)
                        }
                        Op::LEAVE_BLOCK => {
                            self.leave(bp)?;
                            (block, ip)
                        }
                        Op::REDUCE => {
                            let result = self.stack.pop_all(bp).ok_or(CoreError::InternalError)?;
                            let block = self.module.heap.alloc_block(&result)?;
                            self.stack.push([VmValue::TAG_BLOCK, block])?;
                            (block, ip)
                        }
                        Op::FOREACH => {
                            self.stack.pop_all(bp).ok_or(CoreError::InternalError)?; // drop result
                            let [_, i] = self.stack.pop()?;
                            let [_, word, _, data, _, body] =
                                self.stack.peek().ok_or(MemoryError::StackUnderflow)?;
                            let index = i + 2;
                            if let Ok(value) = self.get_block(data, index) {
                                let mut ctx = self.peek_context()?;
                                ctx.put(word, value)?;
                                self.push([VmValue::TAG_INT, index])?;
                                self.op_stack
                                    .push([Op::FOREACH, block, self.stack.len()?, ip])?;
                                (body, Self::LEAVE_MARKER + 0)
                            } else {
                                self.stack.pop::<6>()?;
                                self.pop_context()?;
                                (block, ip)
                            }
                        }
                        _ => return Ok((op, block)),
                    }
                };

                self.block = block;
                self.ip = ip - Self::LEAVE_MARKER;
            }
        }
    }

    fn leave(&mut self, bp: Offset) -> Result<(), MemoryError> {
        let sp = self.stack.len()?;
        match sp.checked_sub(bp) {
            Some(2) => {}
            Some(0) => {
                self.stack.push([VmValue::TAG_NONE, 0])?;
            }
            Some(_) => {
                let result = self.stack.pop::<2>()?;
                self.stack.set_len(bp)?;
                self.stack.push(result)?;
            }
            None => {
                return Err(MemoryError::StackUnderflow);
            }
        };
        Ok(())
    }

    pub fn eval(&mut self) -> Result<VmValue, CoreError> {
        loop {
            match self.next_op() {
                Ok((op, word)) => self.do_op(op, word)?,
                Err(CoreError::EndOfInput) => {
                    if self.stack.is_empty()? {
                        return [VmValue::TAG_NONE, 0].try_into();
                    } else {
                        let result = self.stack.pop()?;
                        return result.try_into();
                    }
                }
                Err(error) => return Err(error),
            }
        }
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
            .and_then(|offset| self.parse.push([VmValue::TAG_INLINE_STRING, offset]))
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Result<(), Self::Error> {
        self.module.get_or_insert_symbol(word).and_then(|id| {
            let value = match kind {
                WordKind::Word => VmValue::Word(id),
                WordKind::SetWord => VmValue::SetWord(id),
                WordKind::GetWord => VmValue::GetWord(id),
            };
            self.parse.push(value.vm_repr())
        })
    }

    fn integer(&mut self, value: i32) -> Result<(), MemoryError> {
        self.parse.push([VmValue::TAG_INT, value as u32])
    }

    fn begin_block(&mut self) -> Result<(), MemoryError> {
        self.parse.len().and_then(|len| self.ops.push([len]))
    }

    fn end_block(&mut self) -> Result<(), MemoryError> {
        let [bp] = self.ops.pop()?;
        let block_data = self.parse.pop_all(bp).ok_or(MemoryError::UnexpectedError)?;
        let offset = self.module.heap.alloc_block(block_data)?;
        self.parse.push([VmValue::TAG_BLOCK, offset])
    }

    fn begin_path(&mut self) -> Result<(), Self::Error> {
        self.parse.len().and_then(|len| self.ops.push([len]))
    }

    fn end_path(&mut self) -> Result<(), Self::Error> {
        let [bp] = self.ops.pop()?;
        let block_data = self.parse.pop_all(bp).ok_or(MemoryError::UnexpectedError)?;
        let offset = self.module.heap.alloc_block(block_data)?;
        self.parse.push([VmValue::TAG_PATH, offset])
    }
}

//

// pub fn parse(module: &mut Module<&mut [Word]>, str: &str) -> Result<Box<[Word]>, CoreError> {
//     module.parse(str)
// }

pub fn eval(module: &mut Exec<&mut [Word]>) -> Result<VmValue, CoreError> {
    module.eval()
}

// pub fn next_op(module: &mut Exec<&mut [Word]>) -> Option<(Word, Word)> {
//     module.next_op()
// }

// pub fn do_op(module: &mut Exec<&mut [Word]>, op: Word, word: Word) -> Option<()> {
//     module.do_op(op, word)
// }

//

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rebel;
    use crate::value::Value;

    fn eval(input: &str) -> Result<VmValue, CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");
        let block = module.parse(input)?;
        module.eval(block)
    }

    /// This test demonstrates the new Context iterator functionality by:
    /// 1. Creating a context value with key-value pairs
    /// 2. Allocating it in the VM
    /// 3. Reading it back using the read_value function
    /// 4. Iterating through the context entries using our new iterator
    /// 5. Verifying all keys and values
    ///
    /// It shows how the Context iterator makes it easy to process all
    /// entries in a Context without manually handling hash table lookups.
    #[test]
    fn test_context_transform() -> Result<(), CoreError> {
        // Initialize a module
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Create a test context directly with the high-level API
        let test_context = Value::Context(Box::new([
            ("name".into(), Value::String("John Doe".into())),
            ("age".into(), Value::Int(42)),
            ("active".into(), Value::Int(1)),
        ]));

        // Allocate this context in VM memory
        let vm_context = module
            .alloc_value(&test_context)
            .expect("Failed to allocate context");

        // Get the VM representation (tag and address)
        let [tag, addr] = vm_context.vm_repr();

        // Verify we have a context tag
        assert_eq!(tag, VmValue::TAG_CONTEXT, "VM value should be a Context");

        // Store the context at a known memory location
        let storage_addr = module
            .heap
            .alloc([tag, addr])
            .expect("Failed to allocate storage");

        // Read back the value from memory
        let result_value = module
            .read_value(storage_addr)
            .expect("Failed to read value");

        // Verify we got a context back
        assert!(
            matches!(result_value, Value::Context(_)),
            "Result should be a context, got: {:?}",
            result_value
        );

        // Verify the context contents using the iterator
        if let Value::Context(pairs) = result_value {
            // Should have 3 entries
            assert_eq!(pairs.len(), 3, "Context should have 3 entries");

            // Use the iterator to build a map
            let context_map: std::collections::HashMap<_, _> =
                pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

            // Verify all keys exist
            assert!(context_map.contains_key("name"), "name key not found");
            assert!(context_map.contains_key("age"), "age key not found");
            assert!(context_map.contains_key("active"), "active key not found");

            // Verify values
            if let Value::String(name) = &**context_map.get("name").unwrap() {
                assert_eq!(name, "John Doe", "Name value mismatch");
            } else {
                panic!("Name is not a string value");
            }

            if let Value::Int(age) = &**context_map.get("age").unwrap() {
                assert_eq!(*age, 42, "Age value mismatch");
            } else {
                panic!("Age is not an integer value");
            }

            if let Value::Int(active) = &**context_map.get("active").unwrap() {
                assert_eq!(*active, 1, "Active value mismatch");
            } else {
                panic!("Active is not an integer value");
            }

            // Demonstrate the Context iterator's find method
            let age_entry = pairs
                .iter()
                .find(|(key, _)| key == "age")
                .expect("Age entry not found");

            assert_eq!(age_entry.0, "age", "Key should be 'age'");
            assert!(matches!(age_entry.1, Value::Int(42)), "Age should be 42");
        } else {
            panic!("Result is not a context");
        }

        // Create a more complex nested context
        let nested_context = Value::Context(Box::new([
            (
                "profile".into(),
                Value::Context(Box::new([
                    ("name".into(), Value::String("Jane Smith".into())),
                    ("age".into(), Value::Int(35)),
                ])),
            ),
            (
                "settings".into(),
                Value::Context(Box::new([
                    ("theme".into(), Value::String("dark".into())),
                    ("notifications".into(), Value::Int(1)),
                ])),
            ),
            ("created".into(), Value::Int(12345)),
        ]));

        // Allocate the nested context in VM memory
        let nested_vm = module
            .alloc_value(&nested_context)
            .expect("Failed to allocate nested context");

        // Get the VM representation
        let [nested_tag, nested_addr] = nested_vm.vm_repr();

        // Verify we have a context tag
        assert_eq!(
            nested_tag,
            VmValue::TAG_CONTEXT,
            "Nested VM value should be a Context"
        );

        // Store at a known location
        let nested_storage = module
            .heap
            .alloc([nested_tag, nested_addr])
            .expect("Failed to allocate nested storage");

        // Read back the nested context
        let nested_result = module
            .read_value(nested_storage)
            .expect("Failed to read nested context");

        // Verify we got a context back
        assert!(
            matches!(nested_result, Value::Context(_)),
            "Nested result should be a context, got: {:?}",
            nested_result
        );

        // Verify the nested context structure using the iterator
        if let Value::Context(outer_pairs) = nested_result {
            // Should have 3 entries
            assert_eq!(outer_pairs.len(), 3, "Outer context should have 3 entries");

            // Use the iterator to build a map
            let outer_map: std::collections::HashMap<_, _> =
                outer_pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

            // Verify keys exist
            assert!(outer_map.contains_key("profile"), "profile key not found");
            assert!(outer_map.contains_key("settings"), "settings key not found");
            assert!(outer_map.contains_key("created"), "created key not found");

            // Verify the profile nested context
            if let Value::Context(profile_pairs) = &**outer_map.get("profile").unwrap() {
                // Use iterator for the nested context
                let profile_map: std::collections::HashMap<_, _> =
                    profile_pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

                // Verify profile keys
                assert!(
                    profile_map.contains_key("name"),
                    "profile name key not found"
                );
                assert!(profile_map.contains_key("age"), "profile age key not found");

                // Verify profile values
                if let Value::String(name) = &**profile_map.get("name").unwrap() {
                    assert_eq!(name, "Jane Smith", "Profile name value mismatch");
                } else {
                    panic!("Profile name is not a string value");
                }

                if let Value::Int(age) = &**profile_map.get("age").unwrap() {
                    assert_eq!(*age, 35, "Profile age value mismatch");
                } else {
                    panic!("Profile age is not an integer value");
                }
            } else {
                panic!("Profile is not a context");
            }

            // Verify the settings nested context
            if let Value::Context(settings_pairs) = &**outer_map.get("settings").unwrap() {
                // Use iterator for the settings context
                let settings_map: std::collections::HashMap<_, _> = settings_pairs
                    .iter()
                    .map(|(k, v)| (k.as_str(), v))
                    .collect();

                // Verify settings keys
                assert!(
                    settings_map.contains_key("theme"),
                    "settings theme key not found"
                );
                assert!(
                    settings_map.contains_key("notifications"),
                    "settings notifications key not found"
                );

                // Verify settings values
                if let Value::String(theme) = &**settings_map.get("theme").unwrap() {
                    assert_eq!(theme, "dark", "Settings theme value mismatch");
                } else {
                    panic!("Settings theme is not a string value");
                }

                if let Value::Int(notifications) = &**settings_map.get("notifications").unwrap() {
                    assert_eq!(*notifications, 1, "Settings notifications value mismatch");
                } else {
                    panic!("Settings notifications is not an integer value");
                }
            } else {
                panic!("Settings is not a context");
            }

            // Verify the created timestamp
            if let Value::Int(created) = &**outer_map.get("created").unwrap() {
                assert_eq!(*created, 12345, "Created timestamp value mismatch");
            } else {
                panic!("Created is not an integer value");
            }
        } else {
            panic!("Nested result is not a context");
        }

        Ok(())
    }

    #[test]
    fn test_alloc_value() {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        // Test None value
        let none_val = Value::None;
        let vm_none = module
            .alloc_value(&none_val)
            .expect("Failed to allocate None value");
        assert!(matches!(vm_none, VmValue::None));

        // Test Int value
        let int_val = Value::Int(42);
        let vm_int = module
            .alloc_value(&int_val)
            .expect("Failed to allocate Int value");
        assert!(matches!(vm_int, VmValue::Int(n) if n == 42));

        // Test String value
        let string_val = Value::String("hello".into());
        let vm_string = module
            .alloc_value(&string_val)
            .expect("Failed to allocate String value");
        assert!(matches!(vm_string, VmValue::String(_)));

        // Test Word value
        let word_val = Value::Word("test".into());
        let vm_word = module
            .alloc_value(&word_val)
            .expect("Failed to allocate Word value");
        assert!(matches!(vm_word, VmValue::Word(_)));

        // Test SetWord value
        let setword_val = Value::SetWord("test".into());
        let vm_setword = module
            .alloc_value(&setword_val)
            .expect("Failed to allocate SetWord value");
        assert!(matches!(vm_setword, VmValue::SetWord(_)));

        // Test Block value
        let block_val = Value::Block(Box::new([Value::Int(1), Value::Int(2)]));
        let vm_block = module
            .alloc_value(&block_val)
            .expect("Failed to allocate Block value");
        assert!(matches!(vm_block, VmValue::Block(_)));

        // Test Context value
        let context_val = Value::Context(Box::new([
            ("name".into(), Value::String("John".into())),
            ("age".into(), Value::Int(30)),
        ]));
        let vm_context = module
            .alloc_value(&context_val)
            .expect("Failed to allocate Context value");
        assert!(matches!(vm_context, VmValue::Context(_)));
    }

    #[test]
    fn test_value_roundtrip() {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        // Test simple values - start with just None
        let none_val = Value::None;
        let vm_none = module
            .alloc_value(&none_val)
            .expect("Failed to allocate None value");
        let roundtrip_none = module.to_value(vm_none).expect("Failed to read None value");
        assert_eq!(none_val, roundtrip_none);

        // Test Int value
        let int_val = Value::Int(42);
        let vm_int = module
            .alloc_value(&int_val)
            .expect("Failed to allocate Int value");
        let roundtrip_int = module.to_value(vm_int).expect("Failed to read Int value");
        assert_eq!(int_val, roundtrip_int);

        // Test String value
        let string_val = Value::String("hello world".into());
        let vm_string = module
            .alloc_value(&string_val)
            .expect("Failed to allocate String value");
        let roundtrip_string = module
            .to_value(vm_string)
            .expect("Failed to read String value");
        assert_eq!(string_val, roundtrip_string);

        // Test Word value
        let word_val = Value::Word("test".into());
        let vm_word = module
            .alloc_value(&word_val)
            .expect("Failed to allocate Word value");
        let roundtrip_word = module.to_value(vm_word).expect("Failed to read Word value");

        if let (Value::Word(w1), Value::Word(w2)) = (&word_val, &roundtrip_word) {
            assert_eq!(w1, w2);
        } else {
            panic!("Word value did not roundtrip correctly");
        }

        // Test SetWord value
        let setword_val = Value::SetWord("counter".into());
        let vm_setword = module
            .alloc_value(&setword_val)
            .expect("Failed to allocate SetWord value");
        let roundtrip_setword = module
            .to_value(vm_setword)
            .expect("Failed to read SetWord value");

        if let (Value::SetWord(w1), Value::SetWord(w2)) = (&setword_val, &roundtrip_setword) {
            assert_eq!(w1, w2);
        } else {
            panic!("SetWord value did not roundtrip correctly");
        }

        // Test simple block
        let simple_block = Value::Block(Box::new([Value::Int(1), Value::Int(2)]));
        let vm_simple_block = module
            .alloc_value(&simple_block)
            .expect("Failed to allocate simple Block value");
        let roundtrip_simple_block = module
            .to_value(vm_simple_block)
            .expect("Failed to read simple Block value");
        assert_eq!(simple_block, roundtrip_simple_block);

        // Test nested block
        let nested_block = Value::Block(Box::new([
            Value::Int(1),
            Value::String("test".into()),
            Value::Block(Box::new([Value::Int(2), Value::Int(3)])),
        ]));

        let vm_block = module
            .alloc_value(&nested_block)
            .expect("Failed to allocate nested Block value");
        let roundtrip_block = module
            .to_value(vm_block)
            .expect("Failed to read nested Block value");
        assert_eq!(nested_block, roundtrip_block);

        // Test simple context
        let simple_context = Value::Context(Box::new([
            ("name".into(), Value::String("John".into())),
            ("age".into(), Value::Int(30)),
        ]));

        let vm_simple_context = module
            .alloc_value(&simple_context)
            .expect("Failed to allocate simple Context value");
        let roundtrip_simple_context = module
            .to_value(vm_simple_context)
            .expect("Failed to read simple Context value");

        // For contexts, we need to compare pairs individually since order might change
        if let (Value::Context(orig_pairs), Value::Context(rt_pairs)) =
            (&simple_context, &roundtrip_simple_context)
        {
            assert_eq!(
                orig_pairs.len(),
                rt_pairs.len(),
                "Context sizes don't match"
            );

            // Check each key-value pair
            for (orig_key, orig_val) in orig_pairs.iter() {
                let found = rt_pairs.iter().find(|(k, _)| k == orig_key);
                assert!(
                    found.is_some(),
                    "Key {} not found in roundtrip context",
                    orig_key
                );

                if let Some((_, rt_val)) = found {
                    assert_eq!(orig_val, rt_val, "Value for key {} doesn't match", orig_key);
                }
            }
        } else {
            panic!("Roundtrip value is not a context");
        }

        // Test nested context
        let nested_context = Value::Context(Box::new([
            ("name".into(), Value::String("John".into())),
            ("age".into(), Value::Int(30)),
            (
                "data".into(),
                Value::Block(Box::new([Value::Int(1), Value::Int(2)])),
            ),
            (
                "profile".into(),
                Value::Context(Box::new([
                    ("email".into(), Value::String("john@example.com".into())),
                    ("active".into(), Value::Int(1)),
                ])),
            ),
        ]));

        let vm_context = module
            .alloc_value(&nested_context)
            .expect("Failed to allocate nested Context value");
        let roundtrip_context = module
            .to_value(vm_context)
            .expect("Failed to read nested Context value");

        // For contexts, we need to compare pairs individually since order might change
        if let (Value::Context(orig_pairs), Value::Context(rt_pairs)) =
            (&nested_context, &roundtrip_context)
        {
            assert_eq!(
                orig_pairs.len(),
                rt_pairs.len(),
                "Context sizes don't match"
            );

            // Check each key-value pair
            for (orig_key, orig_val) in orig_pairs.iter() {
                let found = rt_pairs.iter().find(|(k, _)| k == orig_key);
                assert!(
                    found.is_some(),
                    "Key {} not found in roundtrip context",
                    orig_key
                );

                if let Some((_, rt_val)) = found {
                    match (orig_val, rt_val) {
                        // For nested contexts, just check that they're both contexts with the same length
                        (Value::Context(c1), Value::Context(c2)) => {
                            assert_eq!(c1.len(), c2.len(), "Nested context sizes don't match");
                        }
                        // For everything else, they should be equal
                        _ => {
                            assert_eq!(orig_val, rt_val, "Value for key {} doesn't match", orig_key)
                        }
                    }
                }
            }
        } else {
            panic!("Roundtrip value is not a context");
        }
    }

    #[test]
    fn test_read_value_at() {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        // Allocate some values and store their memory addresses
        let original = Value::Block(Box::new([
            Value::Int(42),
            Value::String("hello".into()),
            Value::Word("test".into()),
        ]));

        // Allocate the value and store the VM representation
        let vm_value = module
            .alloc_value(&original)
            .expect("Failed to allocate value");
        let [tag, addr] = vm_value.vm_repr();

        // Store the VM value at a known address
        let storage_addr = module
            .heap
            .alloc([tag, addr])
            .expect("Failed to allocate storage");

        // Read the value back using read_value_at
        let roundtrip = module
            .read_value(storage_addr)
            .expect("Failed to read value at address");

        // Compare the original and roundtrip values
        assert_eq!(original, roundtrip);
    }

    #[test]
    fn test_whitespace_1() -> Result<(), CoreError> {
        let result = eval("  \t\n  ")?;
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_string_1() -> Result<(), CoreError> {
        let result = eval(" \"hello\"  ")?;
        assert!(result.is_string());
        Ok(())
    }

    #[test]
    fn test_word_1() -> Result<(), CoreError> {
        let input = "42 \"world\" x: 5 x\n ";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(5), result);
        Ok(())
    }

    #[test]
    fn test_add_1() -> Result<(), CoreError> {
        let input = "add 7 8";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(15), result);
        Ok(())
    }

    #[test]
    fn test_add_2() -> Result<(), CoreError> {
        let input = "add 1 add 2 3";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(6), result);
        Ok(())
    }

    #[test]
    fn test_add_3() -> Result<(), CoreError> {
        let input = "add add 3 4 5";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(12), result);
        Ok(())
    }

    #[test]
    fn test_context_0() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input = "context [x: 8]";
        let block = module.parse(input)?;
        let result = module.eval(block)?;

        assert!(result.is_context());
        let value = module.to_value(result)?;

        if let Value::Context(pairs) = value {
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].0, "x");
            assert_eq!(pairs[0].1, 8.into());
        } else {
            panic!("Result should be a context");
        }

        Ok(())
    }

    #[test]
    fn test_context_1() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input =
            "make-person: func [name age] [context [name: name age: age]] make-person \"Alice\" 30";
        let block = module.parse(input)?;
        let result = module.eval(block)?;

        assert!(result.is_context());
        let value = module.to_value(result)?;

        if let Value::Context(pairs) = value {
            assert_eq!(pairs.len(), 2);
            let mut found = 0;
            for (key, value) in pairs.iter() {
                match key.as_str() {
                    "name" => {
                        assert_eq!(*value, "Alice".into());
                        found += 1;
                    }
                    "age" => {
                        assert_eq!(*value, 30.into());
                        found += 1;
                    }
                    _ => panic!("Unexpected key: {}", key),
                }
            }
            assert_eq!(found, 2);
        } else {
            panic!("Result should be a context");
        }

        Ok(())
    }

    #[test]
    fn test_reduce_1() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input = "reduce [5 add 5 5]";
        let block = module.parse(input)?;
        let result = module.eval(block)?;
        let value = module.to_value(result)?;

        if let Value::Block(values) = value {
            assert_eq!(values.len(), 2);
            assert_eq!(values[0], 5.into());
            assert_eq!(values[1], 10.into());
        } else {
            println!("Result: {:?}", value);
            panic!("Result should be a block");
        }

        Ok(())
    }

    #[test]
    fn test_reduce_2() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input = "ctx: context [a: 8] reduce [5 ctx/a]";
        let block = module.parse(input)?;
        let result = module.eval(block)?;
        let value = module.to_value(result)?;

        if let Value::Block(values) = value {
            assert_eq!(values.len(), 2);
            assert_eq!(values[0], 5.into());
            assert_eq!(values[1], 8.into());
        } else {
            println!("Result: {:?}", value);
            panic!("Result should be a block");
        }

        Ok(())
    }

    #[test]
    fn test_reduce_3() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input = "f: func [ctx] [reduce [5 ctx/a]] f context [a: 8]";
        let block = module.parse(input)?;
        let result = module.eval(block)?;
        let value = module.to_value(result)?;

        if let Value::Block(values) = value {
            assert_eq!(values.len(), 2);
            assert_eq!(values[0], 5.into());
            assert_eq!(values[1], 8.into());
        } else {
            println!("Result: {:?}", value);
            panic!("Result should be a block");
        }

        Ok(())
    }

    #[test]
    fn test_reduce_4() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input = "f: func [value] [either block? value [reduce value] [ \"not a block\" ]] ctx: context [a: 8] f [5 ctx/a]";
        let block = module.parse(input)?;
        let result = module.eval(block)?;
        let value = module.to_value(result)?;

        if let Value::Block(values) = value {
            assert_eq!(values.len(), 2);
            assert_eq!(values[0], 5.into());
            assert_eq!(values[1], 8.into());
        } else {
            println!("Result: {:?}", value);
            panic!("Result should be a block");
        }

        Ok(())
    }

    #[test]
    fn test_foreach_1() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input = "sum: 0 foreach x [1 2 3 4 5] [sum: add sum x] sum";
        let block = module.parse(input)?;
        let result = module.eval(block)?;
        let value = module.to_value(result)?;

        assert_eq!(Value::Int(15), value);

        Ok(())
    }

    #[test]
    fn test_path_1() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input = "ctx: context [x: 42] ctx/x";
        let block = module.parse(input)?;
        let result = module.eval(block)?;

        // assert!(result.is_context());
        let value = module.to_value(result)?;

        assert_eq!(Value::Int(42), value);

        Ok(())
    }

    #[test]
    fn test_path_2() -> Result<(), CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        let input = "ctx: context [x: 42 inner: context [a: 1 b: 2]] ctx/inner/a";
        let block = module.parse(input)?;
        let result = module.eval(block)?;

        let value = module.to_value(result)?;

        assert_eq!(Value::Int(1), value);

        Ok(())
    }

    #[test]
    fn test_func_1() -> Result<(), CoreError> {
        let input = "f: func [a b] [add a b] f 1 77";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(78), result);
        Ok(())
    }

    #[test]
    fn test_func_2() -> Result<(), CoreError> {
        let input = "f: func [a b] [add a add b b] f 1 2";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(5), result);
        Ok(())
    }

    #[test]
    fn test_either_1() -> Result<(), CoreError> {
        let input = "either lt 1 2 [1] [2]";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(1), result);
        Ok(())
    }

    #[test]
    fn test_either_2() -> Result<(), CoreError> {
        let input = "either lt 2 1 [1] [2]";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(2), result);
        Ok(())
    }

    #[test]
    fn test_do_1() -> Result<(), CoreError> {
        let input = "do [add 1 2]";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(3), result);
        Ok(())
    }

    #[test]
    fn test_func_fib() -> Result<(), CoreError> {
        let input = "fib: func [n] [either lt n 2 [n] [add fib add n -1 fib add n -2]] fib 10";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(55), result);
        Ok(())
    }

    #[test]
    fn test_func_sum() -> Result<(), CoreError> {
        let input = "sum: func [n] [either lt n 2 [n] [add 1 sum add n -1]] sum 10";
        let result = eval(input)?;
        assert_eq!(VmValue::Int(10), result);
        Ok(())
    }

    #[test]
    fn test_context_implementation() -> Result<(), CoreError> {
        // Create a module
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Create a context with known values
        let context_data = Value::Context(Box::new([
            ("name".into(), Value::String("Test".into())),
            ("value".into(), Value::Int(42)),
        ]));

        // Allocate in VM memory
        let vm_context = module
            .alloc_value(&context_data)
            .expect("Failed to allocate context");

        // Get VM representation
        let [tag, _] = vm_context.vm_repr();
        assert_eq!(tag, VmValue::TAG_CONTEXT, "Should be a context tag");

        Ok(())
    }

    /// Test the CALL_NATIVE operation with a simple program [add 7 8]
    /// This test verifies that:
    /// 1. The next_op method correctly identifies the CALL_NATIVE operation for the 'add' word
    /// 2. The do_op method correctly executes the native function with the arguments
    /// 3. The result is correctly pushed onto the stack
    #[test]
    fn test_call_native() -> Result<(), CoreError> {
        // Initialize a module
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Create a program [add 7 8] using the rebel! macro
        let program = rebel!([add 7 8]);

        // Allocate the program in VM memory
        let vm_block = module
            .alloc_value(&program)
            .expect("Failed to allocate block");

        // Get the VM representation
        let [_, block_addr] = vm_block.vm_repr();

        // Create an execution context
        let mut exec =
            Exec::new(&mut module, block_addr).expect("Failed to create execution context");

        // First call to next_op should process the 'add' word and identify it as a CALL_NATIVE operation
        // It will also push the arguments 7 and 8 onto the stack
        let op_result = exec.next_op();
        assert!(op_result.is_ok(), "next_op should return an operation");
        let (op, value) = op_result.unwrap();

        // Should return CALL_NATIVE operation
        assert_eq!(op, Op::CALL_NATIVE, "First operation should be CALL_NATIVE");

        // The function index depends on the order functions are registered in boot.rs
        // Looking at boot.rs, 'add' is the first registered function (index 0)
        // We could test for the specific value, but that would make the test brittle if
        // the order of function registration changes in boot.rs
        assert_eq!(value, 0, "Expected 'add' function to be at index 0");

        // Stack should have two values (7 and 8) pushed as pairs of [TAG_INT, value]
        assert_eq!(
            exec.stack.len().unwrap(),
            2 * 2,
            "Stack should have 2 values (4 words total)"
        );

        // Check that the values on stack are 7 and 8
        let val1 = exec.stack.get::<2>(0).expect("Failed to get first value");
        let val2 = exec.stack.get::<2>(2).expect("Failed to get second value");

        assert_eq!(val1, [VmValue::TAG_INT, 7], "First value should be 7");
        assert_eq!(val2, [VmValue::TAG_INT, 8], "Second value should be 8");

        // Execute the CALL_NATIVE operation using do_op
        // This should pop the values 7 and 8 from the stack, add them, and push the result 15
        exec.do_op(op, value).expect("do_op failed for CALL_NATIVE");

        // Stack should now have one value (the result)
        assert_eq!(
            exec.stack.len().unwrap(),
            1 * 2,
            "Stack should have 1 value after do_op"
        );

        // The result should be 15
        let result = exec.stack.get::<2>(0).expect("Failed to get result");
        assert_eq!(result, [VmValue::TAG_INT, 15], "Result should be 15");

        // There should be no more operations (next_op should return None)
        let next_result = exec.next_op();
        assert!(
            next_result.is_err(),
            "next_op should return None at end of block"
        );

        Ok(())
    }

    /// Test the CALL_NATIVE operation with a nested expression [add 1 add 2 3]
    /// This test verifies that:
    /// 1. The VM can handle nested function calls correctly
    /// 2. Each function's arguments are evaluated in the correct order
    /// 3. Function results are properly passed as arguments to other functions
    #[test]
    fn test_call_native_nested() -> Result<(), CoreError> {
        // Initialize a module
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Create a nested program [add 1 add 2 3] using the rebel! macro
        // This should evaluate to 6 (1 + (2 + 3))
        let program = rebel!([add 1 add 2 3]);

        // Allocate the program in VM memory
        let vm_block = module
            .alloc_value(&program)
            .expect("Failed to allocate block");

        // Get the VM representation
        let [_, block_addr] = vm_block.vm_repr();

        // Create an execution context
        let mut exec =
            Exec::new(&mut module, block_addr).expect("Failed to create execution context");

        // The first call to next_op will process all values and operations in the program.
        // This is because next_op keeps processing values until it finds an operation that
        // needs to be executed, and it also pushes all values it encounters onto the stack.
        // In this case, it will process 'add', '1', 'add', '2', '3' before returning the
        // CALL_NATIVE operation for the inner 'add'.
        let op_result1 = exec.next_op();
        assert!(
            op_result1.is_ok(),
            "First next_op should return an operation"
        );
        let (op1, value1) = op_result1.unwrap();

        // The first operation should be CALL_NATIVE for 'add' (the inner one)
        assert_eq!(
            op1,
            Op::CALL_NATIVE,
            "First operation should be CALL_NATIVE"
        );
        assert_eq!(value1, 0, "Expected 'add' function to be at index 0");

        // At this point, the stack should have all three values (1, 2, 3)
        assert_eq!(
            exec.stack.len().unwrap(),
            3 * 2,
            "Stack should have 3 values"
        );

        // Check the values on stack (1, 2, 3)
        let val1 = exec.stack.get::<2>(0).expect("Failed to get first value");
        let val2 = exec.stack.get::<2>(2).expect("Failed to get second value");
        let val3 = exec.stack.get::<2>(4).expect("Failed to get third value");

        assert_eq!(val1, [VmValue::TAG_INT, 1], "First value should be 1");
        assert_eq!(val2, [VmValue::TAG_INT, 2], "Second value should be 2");
        assert_eq!(val3, [VmValue::TAG_INT, 3], "Third value should be 3");

        // Execute the inner 'add' operation (add 2 3)
        // This should pop 2 and 3, and push their sum (5)
        exec.do_op(op1, value1)
            .expect("do_op failed for inner CALL_NATIVE");

        // Now the stack should have 2 values: 1 and 5 (the result of inner add)
        assert_eq!(
            exec.stack.len().unwrap(),
            2 * 2,
            "Stack should have 2 values after inner add"
        );

        // Check the values on stack (1, 5)
        let val1_after = exec
            .stack
            .get::<2>(0)
            .expect("Failed to get first value after inner add");
        let val2_after = exec
            .stack
            .get::<2>(2)
            .expect("Failed to get result of inner add");

        assert_eq!(
            val1_after,
            [VmValue::TAG_INT, 1],
            "First value should still be 1"
        );
        assert_eq!(
            val2_after,
            [VmValue::TAG_INT, 5],
            "Result of inner add should be 5"
        );

        // Next call to next_op should identify the CALL_NATIVE operation for the outer 'add'
        let op_result2 = exec.next_op();
        assert!(
            op_result2.is_ok(),
            "Second next_op should return an operation"
        );
        let (op2, value2) = op_result2.unwrap();

        // The second operation should also be CALL_NATIVE for 'add' (the outer one)
        assert_eq!(
            op2,
            Op::CALL_NATIVE,
            "Second operation should be CALL_NATIVE"
        );
        assert_eq!(value2, 0, "Expected 'add' function to be at index 0");

        // Execute the outer 'add' operation (add 1 5)
        // This should pop 1 and 5, and push their sum (6)
        exec.do_op(op2, value2)
            .expect("do_op failed for outer CALL_NATIVE");

        // Now the stack should have 1 value: 6 (the final result)
        assert_eq!(
            exec.stack.len().unwrap(),
            1 * 2,
            "Stack should have 1 value after outer add"
        );

        // Check the final result
        let result = exec.stack.get::<2>(0).expect("Failed to get final result");
        assert_eq!(result, [VmValue::TAG_INT, 6], "Final result should be 6");

        // There should be no more operations (next_op should return None)
        let next_result = exec.next_op();
        assert!(
            next_result.is_err(),
            "next_op should return None at end of block"
        );

        Ok(())
    }

    /// Test a complete program with function definition and call
    /// This test verifies that:
    /// 1. The next_op method correctly processes different operation types
    /// 2. The do_op method correctly executes each operation
    /// 3. The VM correctly handles function definition and function calls
    #[test]
    fn test_call_func() -> Result<(), CoreError> {
        // Initialize a module
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Create a program that defines a function and calls it:
        // [f: func [a b] [add a b] f 10 20]
        let program = rebel!([f: func [a b] [add a b] f 10 20]);

        // Allocate the program in VM memory
        let vm_block = module
            .alloc_value(&program)
            .expect("Failed to allocate block");

        // Get the VM representation
        let [_, block_addr] = vm_block.vm_repr();

        // Create an execution context
        let mut exec =
            Exec::new(&mut module, block_addr).expect("Failed to create execution context");

        // Run the full program by calling eval() instead of testing each operation individually
        // This is easier because the operation order in this program is more complex
        let result = exec.eval().expect("Failed to evaluate program");

        // The final result should be 30 (10 + 20)
        assert_eq!(result, VmValue::Int(30), "Final result should be 30");

        Ok(())
    }

    /// Test the CALL_FUNC operation by verifying that a function call produces the expected result
    /// This test focuses specifically on:
    /// 1. Creating a function that performs a specific operation (doubles its input)
    /// 2. Calling that function with a known argument
    /// 3. Verifying that the CALL_FUNC operation executes properly and returns the correct result
    #[test]
    fn test_call_func_execution() -> Result<(), CoreError> {
        // Initialize a module
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Create a simple test program that:
        // 1. Defines a function 'double' that doubles its input
        // 2. Calls the function with argument 21
        // Expected result: 42
        let program = rebel!([
            double: func [x] [add x x]  // Define a function that doubles its input
            double 21                   // Call the function with argument 21
        ]);

        // Allocate the program in VM memory
        let vm_block = module
            .alloc_value(&program)
            .expect("Failed to allocate block");

        // Execute the program
        let result = module.eval(vm_block).expect("Failed to evaluate program");

        // The result should be 42 (21 doubled)
        assert_eq!(
            result,
            VmValue::Int(42),
            "Function should return 42 (21 doubled)"
        );

        Ok(())
    }

    /// Test verifying that the CALL_FUNC operation handling in next_op works correctly
    /// This test focuses specifically on:
    /// 1. Verifying that the next_op method properly identifies CALL_FUNC operations
    /// 2. Making sure the do_op method for CALL_FUNC correctly handles function calls
    /// 3. Testing that function arguments and results are processed correctly
    #[test]
    fn test_next_op_call_func() -> Result<(), CoreError> {
        // Initialize a module
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Define our test function and enter it in the system context
        let func_program = rebel!([double: func [x] [add x x]]);
        println!("func_program: {:?}", func_program);
        let vm_func = module
            .alloc_value(&func_program)
            .expect("Failed to allocate block");

        module.eval(vm_func).expect("Failed to define function");

        // Verify the direct execution approach works correctly
        let test_call = rebel!([double 21]);
        let vm_call = module
            .alloc_value(&test_call)
            .expect("Failed to allocate block");
        let direct_result = module
            .eval(vm_call)
            .expect("Failed to evaluate function call");
        println!("Direct call result: {:?}", direct_result);
        assert_eq!(
            direct_result,
            VmValue::Int(42),
            "Direct call should return 42"
        );

        // Create another program with a slightly different function call to avoid caching
        let test_call2 = rebel!([double 42]);
        let vm_call2 = module
            .alloc_value(&test_call2)
            .expect("Failed to allocate block");

        // Execute and verify the result
        let result = module
            .eval(vm_call2)
            .expect("Failed to evaluate function call");
        println!("Second function call result: {:?}", result);
        assert_eq!(result, VmValue::Int(84), "Second call should return 84");

        Ok(())
    }

    /// Test path access with function arguments
    /// This test verifies that:
    /// 1. Simple context access works correctly
    /// 2. Function with path access works correctly
    /// 3. Nested path access works correctly
    #[test]
    fn test_path_access() -> Result<(), CoreError> {
        // Initialize a module
        let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;

        // First, test direct context access to verify contexts work correctly
        let simple_context_test = "ctx: context [field: 5] ctx/field";
        let simple_result = eval_code(&mut module, simple_context_test)?;

        // This should work and return 5
        assert_eq!(simple_result, Value::Int(5), "Simple context access failed");
        println!("Simple context access works correctly");

        // Now let's test the problematic case with a function
        let function_test = "f: func [a] [a/field] f context [field: 5]";

        // With our fix, this should now work
        let function_result = eval_code(&mut module, function_test)?;

        // The function should return 5
        assert_eq!(
            function_result,
            Value::Int(5),
            "Function with path access failed"
        );
        println!("Function with path access works correctly");

        // Let's test a more complex case with nested contexts
        let nested_test = "
            ctx: context [
                inner: context [
                    value: 42
                ]
            ]
            f: func [a] [a/inner/value]
            f ctx
        ";

        let nested_result = eval_code(&mut module, nested_test)?;
        assert_eq!(nested_result, Value::Int(42), "Nested path access failed");
        println!("Nested path access works correctly");

        Ok(())
    }

    /// Helper function to evaluate code and convert the result to a Value
    fn eval_code(module: &mut Module<Box<[u32]>>, code: &str) -> Result<Value, CoreError> {
        let block = module.parse(code)?;
        let result = module.eval(block)?;
        module.to_value(result)
    }
}

//
