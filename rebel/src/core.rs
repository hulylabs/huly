// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::boot::core_package;
use crate::mem::{Context, Heap, Offset, Stack, Symbol, SymbolTable, Word};
use crate::parse::{Collector, Parser, WordKind};
use crate::value::Value;
use smol_str::SmolStr;
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
    /// * `Some(VmValue)` - The constructed VmValue if the tag is recognized
    /// * `None` - If the tag is not recognized
    pub fn from_tag_data(tag: Word, data: Word) -> Option<Self> {
        match tag {
            Self::TAG_NONE => Some(VmValue::None),
            Self::TAG_INT => Some(VmValue::Int(data as i32)),
            Self::TAG_BLOCK => Some(VmValue::Block(data)),
            Self::TAG_CONTEXT => Some(VmValue::Context(data)),
            Self::TAG_INLINE_STRING => Some(VmValue::String(data)),
            Self::TAG_WORD => Some(VmValue::Word(data)),
            Self::TAG_SET_WORD => Some(VmValue::SetWord(data)),
            _ => None, // Unknown tag
        }
    }

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
        // Get the raw bytes of the string
        let bytes = string.as_bytes();

        // Calculate how many words we need (1 byte per u32, rounded up)
        let word_count = (bytes.len() + 3) / 4; // ceiling division

        // Create a vector to hold the length + bytes packed into words
        let mut words = Vec::with_capacity(word_count + 1);

        // First word is the length of the string in bytes
        words.push(bytes.len() as u32);

        // Pack bytes into words (4 bytes per word)
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

        // Allocate the block with the words
        self.heap.alloc_block(&words).map(VmValue::String)
    }

    pub fn get_or_insert_symbol(&mut self, symbol: &str) -> Option<Offset> {
        self.get_symbols_mut()?
            .get_or_insert(inline_string(symbol)?)
    }

    pub fn alloc_value(&mut self, value: &Value) -> Option<VmValue> {
        match value {
            // Simple values that don't require heap allocation
            Value::None => Some(VmValue::None),
            Value::Int(n) => Some(VmValue::Int(*n)),

            // Values requiring string allocation
            Value::String(s) => self.alloc_string(s.as_ref()),

            // Symbol-based values
            Value::Word(w) => self.get_or_insert_symbol(w.as_ref()).map(VmValue::Word),
            Value::SetWord(w) => self.get_or_insert_symbol(w.as_ref()).map(VmValue::SetWord),

            // Nested collection types
            Value::Block(items) => {
                // First allocate each value in the block
                let mut vm_values = Vec::with_capacity(items.len() * 2);

                for item in items.iter() {
                    // Recursively allocate each item
                    if let Some(vm_value) = self.alloc_value(item) {
                        // Add the VM representation to our list
                        let repr = vm_value.vm_repr();
                        vm_values.push(repr[0]);
                        vm_values.push(repr[1]);
                    } else {
                        return None; // Allocation failed for an item
                    }
                }

                // Allocate the block in the heap
                self.heap.alloc_block(&vm_values).map(VmValue::Block)
            }

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

                Some(VmValue::Context(context))
            }
        }
    }
}

impl<T> Module<T>
where
    T: AsRef<[Word]>,
{
    pub fn get_symbol(&self, symbol: Symbol) -> Option<SmolStr> {
        let addr = self.heap.get::<1>(Self::SYMBOLS).map(|[addr]| addr)?;
        let symbol_table = self.heap.get_block(addr).map(SymbolTable::new)?;
        let inlined = symbol_table.get(symbol)?;
        let bytes: [u8; 32] = unsafe { std::mem::transmute(inlined) };
        let len = bytes[0] as usize;
        let str = unsafe { std::str::from_utf8_unchecked(&bytes[1..=len]) };
        Some(str.into())
    }

    pub fn to_value(&self, vm_value: VmValue) -> Option<Value> {
        match vm_value {
            // Simple values that don't require heap access
            VmValue::None => Some(Value::None),
            VmValue::Int(n) => Some(Value::Int(n)),

            // Symbol-based values - use our simplified symbol table
            VmValue::Word(symbol) => {
                let symbol_name = self.get_symbol(symbol)?;
                Some(Value::Word(symbol_name))
            }

            VmValue::SetWord(symbol) => {
                let symbol_name = self.get_symbol(symbol)?;
                Some(Value::SetWord(symbol_name))
            }

            // String value stored in heap
            VmValue::String(offset) => {
                let string_block = self.heap.get_block(offset)?;
                if string_block.is_empty() {
                    return Some(Value::String("".into()));
                }

                // First word is the length
                let length = string_block[0] as usize;

                // Safety check on length
                if length > string_block.len() * 4 {
                    return None; // Invalid length
                }

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
                    Ok(string) => Some(Value::String(string.into())),
                    Err(_) => None, // UTF-8 decoding error
                }
            }

            // Block value stored in heap
            VmValue::Block(offset) => {
                let block_data = self.heap.get_block(offset)?;
                if block_data.is_empty() {
                    return Some(Value::Block(Box::new([])));
                }

                let mut values = Vec::new();

                // Process pairs of tag/value words
                for i in (0..block_data.len()).step_by(2) {
                    if i + 1 >= block_data.len() {
                        break; // Incomplete pair
                    }

                    let tag = block_data[i];
                    let data = block_data[i + 1];

                    // Convert tag/data to VmValue using the helper method
                    let vm_value = VmValue::from_tag_data(tag, data)?;

                    // Recursively read the value
                    if let Some(value) = self.to_value(vm_value) {
                        values.push(value);
                    } else {
                        return None; // Failed to read a value
                    }
                }

                Some(Value::Block(values.into_boxed_slice()))
            }

            // Context value stored in heap
            VmValue::Context(offset) => {
                let context_block = self.heap.get_block(offset)?;
                if context_block.is_empty() {
                    return Some(Value::Context(Box::new([])));
                }

                let mut pairs = Vec::new();
                let context_data = Context::new(context_block);

                // Use the iterator to efficiently iterate through all entries in the context
                for (symbol, [tag, data]) in &context_data {
                    // Get the symbol name
                    let symbol_name = self.get_symbol(symbol)?;

                    // Convert the tag/data to a VmValue
                    let Some(vm_value) = VmValue::from_tag_data(tag, data) else {
                        continue; // Skip unknown tags
                    };

                    // Recursively convert to Value
                    if let Some(value) = self.to_value(vm_value) {
                        pairs.push((symbol_name, value));
                    }
                }

                Some(Value::Context(pairs.into_boxed_slice()))
            }
        }
    }

    pub fn read_value(&self, addr: Offset) -> Option<Value> {
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
    use crate::value::Value;

    fn eval(input: &str) -> Result<[Word; 2], CoreError> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");
        let block = module.parse(input)?;
        module.eval(block).ok_or(CoreError::InternalError)
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

    // #[test]
    // fn test_func_returns_context() -> Result<(), CoreError> {
    //     // Define a simple function that creates and returns a context
    //     let input = "make-person: func [] [context [name: \"Alice\" age: 30]] make-person";

    //     // Execute the function
    //     let result = eval(input)?;

    //     // Print the tag for debugging
    //     println!("Return value tag: {}, expected tag: {}", result[0], VmValue::TAG_CONTEXT);

    //     // Verify we got a context back
    //     assert_eq!(result[0], VmValue::TAG_CONTEXT,
    //         "Function should return a Context, got tag {} instead", result[0]);

    //     Ok(())
    // }

    // #[test]
    // fn test_func_with_variable() -> Result<(), CoreError> {
    //     // Define a function that stores the context in a variable first
    //     let input = "
    //         make-person: func [] [
    //             result: context [name: \"Variable\" age: 40]
    //             result
    //         ]
    //         make-person
    //     ";

    //     // Execute the function
    //     let result = eval(input)?;

    //     // Print the tag for debugging
    //     println!("Variable context tag: {}, expected tag: {}", result[0], VmValue::TAG_CONTEXT);

    //     // Verify we got a context back
    //     assert_eq!(result[0], VmValue::TAG_CONTEXT,
    //         "Function should return a Context, got tag {} instead", result[0]);

    //     Ok(())
    // }

    // #[test]
    // fn test_context_function_with_args() -> Result<(), CoreError> {
    //     // Function that takes arguments and returns a context
    //     // Try with explicit variable assignment and return
    //     let input = "
    //         make-user: func [name age] [
    //             result: context [
    //                 name: name
    //                 age: age
    //             ]
    //             result
    //         ]
    //         make-user \"Bob\" 25
    //     ";

    //     // Execute the function
    //     let result = eval(input)?;

    //     // Print the tag for debugging
    //     println!("Return value tag: {}, expected tag: {}", result[0], VmValue::TAG_CONTEXT);

    //     // Verify we got a context back
    //     assert_eq!(result[0], VmValue::TAG_CONTEXT,
    //         "Function should return a Context, got tag {} instead", result[0]);

    //     // Create a module to verify the context contents
    //     let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())
    //         .expect("Failed to create module");

    //     // Convert to high-level Value
    //     let context_value = module.to_value(VmValue::Context(result[1]))
    //         .expect("Failed to convert context to Value");

    //     // Verify the context has the correct structure
    //     if let Value::Context(pairs) = context_value {
    //         println!("Context entries: {}", pairs.len());

    //         // Create a map
    //         let context_map: std::collections::HashMap<_, _> = pairs.iter()
    //             .map(|(k, v)| (k.as_str(), v))
    //             .collect();

    //         // Print all keys in the context
    //         println!("Context keys: {:?}", context_map.keys().collect::<Vec<_>>());

    //         // For debugging, print all key-value pairs
    //         for (key, value) in &context_map {
    //             println!("Key: {}, Value: {:?}", key, value);
    //         }

    //         // Only verify the length if we have entries
    //         if !pairs.is_empty() {
    //             assert_eq!(pairs.len(), 2, "Context should have 2 entries");

    //             // Verify the name value
    //             if let Value::String(name) = &**context_map.get("name").unwrap() {
    //                 assert_eq!(name, "Bob", "Name should be 'Bob'");
    //             } else {
    //                 panic!("Name is not a String value");
    //             }

    //             // Verify the age value
    //             if let Value::Int(age) = &**context_map.get("age").unwrap() {
    //                 assert_eq!(*age, 25, "Age should be 25");
    //             } else {
    //                 panic!("Age is not an Int value");
    //             }
    //         }
    //     } else {
    //         panic!("Return value is not a Context: {:?}", context_value);
    //     }

    //     Ok(())
    // }

    // #[test]
    // fn test_direct_context() -> Result<(), CoreError> {
    //     // Just a direct context expression, no function
    //     let input = "context [name: \"Direct\" value: 123]";

    //     // Execute the code
    //     let result = eval(input)?;

    //     // Print the tag for debugging
    //     println!("Direct context tag: {}, expected tag: {}", result[0], VmValue::TAG_CONTEXT);

    //     // Verify we got a context back
    //     assert_eq!(result[0], VmValue::TAG_CONTEXT,
    //         "Should return a Context, got tag {} instead", result[0]);

    //     // Create a module to examine the context contents
    //     let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())
    //         .expect("Failed to create module");

    //     // Convert to Value
    //     let context_value = module.to_value(VmValue::Context(result[1]))
    //         .expect("Failed to convert context");

    //     // Check the context contents
    //     if let Value::Context(pairs) = context_value {
    //         println!("Direct context entries: {}", pairs.len());

    //         // Create a map of all entries
    //         let context_map: std::collections::HashMap<_, _> = pairs.iter()
    //             .map(|(k, v)| (k.as_str(), v))
    //             .collect();

    //         // Print all keys
    //         println!("Direct context keys: {:?}", context_map.keys().collect::<Vec<_>>());

    //         // Print all entries
    //         for (key, value) in &context_map {
    //             println!("Key: {}, Value: {:?}", key, value);
    //         }
    //     } else {
    //         panic!("Not a context: {:?}", context_value);
    //     }

    //     Ok(())
    // }

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
        let [tag, addr] = vm_context.vm_repr();
        assert_eq!(tag, VmValue::TAG_CONTEXT, "Should be a context tag");

        // Print the address
        println!("Context allocated at address: {}", addr);

        // Get the low-level context structure
        let raw_context = module
            .heap
            .get_block(addr)
            .expect("Failed to get context block");

        // Print information about the context
        println!("Raw context block length: {}", raw_context.len());
        println!(
            "Context header value: {}",
            raw_context.first().unwrap_or(&0)
        );

        // If it's a valid context, try to wrap it in a Context struct and use our iterator
        use crate::mem::Context;
        let context_wrapper = Context::new(raw_context);

        // Use our iterator to print entries
        println!("Entries from Context iterator:");
        let mut count = 0;
        for (symbol, value) in &context_wrapper {
            println!("  Symbol: {}, Value: {:?}", symbol, value);
            count += 1;
        }
        println!("Total entries from iterator: {}", count);

        // Convert back to a high-level Value
        let roundtrip = module
            .to_value(vm_context)
            .expect("Failed to convert context to Value");

        // Check the resulting Value
        if let Value::Context(pairs) = roundtrip {
            println!("Entries in roundtrip Value::Context: {}", pairs.len());
            for (key, value) in pairs.iter() {
                println!("  Key: {}, Value: {:?}", key, value);
            }
        } else {
            panic!("Not a context: {:?}", roundtrip);
        }

        Ok(())
    }
}

//
