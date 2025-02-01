// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::hash::fast_hash;
use crate::parser::{ParseError, ParseIterator};
use thiserror::Error;

pub type Address = u32;
pub type Symbol = Address;

#[derive(Debug, Clone, Copy)]
pub struct Context(Address);

#[derive(Debug, Clone, Copy)]
pub struct Block(Address);

pub type NativeFn = fn(&mut Memory, usize) -> Result<(), MemoryError>;

pub struct Module {
    pub procs: &'static [(&'static str, NativeFn)],
}

#[derive(Debug, Clone, Copy)]
pub struct Value {
    tag: u32,
    value: u32,
}

impl Value {
    pub const INT: u32 = 0x0;
    const STRING: u32 = 0x1;
    pub const BLOCK: u32 = 0x2;
    const CONTEXT: u32 = 0x3;
    pub const WORD: u32 = 0x4;
    pub const SET_WORD: u32 = 0x5;
    pub const NATIVE_FN: u32 = 0x6;
    const TAG_NONE: u32 = 0x7;

    pub const NONE: Value = Value {
        tag: Self::TAG_NONE,
        value: 0,
    };

    fn native_fn(id: u32) -> Self {
        Value {
            tag: Self::NATIVE_FN,
            value: id,
        }
    }

    //

    fn eval_read(self, memory: &mut Memory) -> Option<()> {
        match self.tag {
            Value::NATIVE_FN | Value::SET_WORD => memory.push_op(self),
            _ => memory.push(self),
        }
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        let value = value as u32;
        Value {
            tag: Self::INT,
            value,
        }
    }
}

impl TryFrom<Value> for i32 {
    type Error = MemoryError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value.tag {
            Value::INT => Ok(value.value as i32),
            _ => Err(MemoryError::TypeMismatch),
        }
    }
}

//

impl Block {
    pub fn len(&self, memory: &Memory) -> Option<usize> {
        memory.heap.get(self.0 as usize).map(|&len| len as usize)
    }

    pub fn get(&self, memory: &Memory, index: usize) -> Option<Value> {
        let addr = self.0 as usize + index * 2 + 1;
        memory.heap.get(addr..addr + 2).map(|mem| Value {
            tag: mem[0],
            value: mem[1],
        })
    }

    pub fn parse(memory: &mut Memory, input: &str) -> Result<Block, MemoryError> {
        let mut iter = ParseIterator::new(input, memory);
        iter.create_block()
    }

    fn eval<'a>(self, memory: &'a mut Memory) -> Result<(Context, Value), MemoryError> {
        if let Some(len) = self.len(memory) {
            for i in 0..len {
                if let Some(value) = self.get(memory, i) {
                    match value.tag {
                        Value::WORD => {
                            let word_value = ctx
                                .get(memory, value.value)
                                .ok_or(MemoryError::WordNotFound(value.value))?;
                            word_value
                                .eval_read(memory)
                                .ok_or(MemoryError::MemoryAccessError)?;
                        }
                        _ => value
                            .eval_read(memory)
                            .ok_or(MemoryError::MemoryAccessError)?,
                    }
                }
            }
        }

        while let Some(op) = memory.pop_op() {
            match *op {
                [Value::NATIVE_FN, id, bp] => memory.call(id, bp)?,
                [Value::SET_WORD, sym, bp] => {
                    let value = memory
                        .peek(bp as usize)
                        .ok_or(MemoryError::StackUnderflow)?;
                    ctx = ctx.add(memory, sym, value)?
                }
                _ => return Err(MemoryError::InternalError),
            }
        }

        Ok((ctx, memory.clear_stack().unwrap_or(Value::NONE)))
    }
}

impl TryFrom<Value> for Block {
    type Error = MemoryError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value.tag {
            Value::BLOCK => Ok(Block(value.value)),
            _ => Err(MemoryError::TypeMismatch),
        }
    }
}

impl From<Block> for Value {
    fn from(block: Block) -> Self {
        Value {
            tag: Value::BLOCK,
            value: block.0,
        }
    }
}

impl Context {
    fn head(self, memory: &mut Memory) -> Option<Address> {
        memory.heap.get(self.0 as usize).copied()
    }

    /// Each context entry stored as below (u32 values):
    /// [symbol, next, tag, value]
    pub fn add(self, memory: &mut Memory, symbol: Symbol, value: Value) -> Option<Self> {
        let mut addr = self.0;
        while addr != 0 {
            let a = addr as usize;
            let entry = memory.heap.get(a..a + 2)?;
            if entry[0] == symbol {
                return memory.heap.get_mut(a + 2..a + 4).map(|mem| {
                    mem[0] = value.tag;
                    mem[1] = value.value;
                    self
                });
            }
            addr = entry[1];
        }
        memory
            .heap
            .get_mut(memory.heap_ptr..memory.heap_ptr + 4)
            .map(|new_entry| {
                new_entry[0] = symbol;
                new_entry[1] = self.0;
                new_entry[2] = value.tag;
                new_entry[3] = value.value;
                let address = memory.heap_ptr as Address;
                memory.heap_ptr += 4;
                Context(address)
            })
    }

    pub fn get(&self, memory: &Memory, symbol: Symbol) -> Option<Value> {
        let mut addr = self.0;
        while addr != 0 {
            if let Some(entry) = memory.heap.get(addr as usize..addr as usize + 4) {
                if entry[0] == symbol {
                    return Some(Value {
                        tag: entry[2],
                        value: entry[3],
                    });
                }
                addr = entry[1];
            } else {
                return None;
            }
        }
        None
    }
}

impl TryFrom<Value> for Context {
    type Error = MemoryError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value.tag {
            Value::CONTEXT => Ok(Context(value.value)),
            _ => Err(MemoryError::TypeMismatch),
        }
    }
}

//

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Out of memory")]
    OutOfMemory,
    #[error("String too long for inlined storage")]
    StringTooLong,
    #[error("Out of symbol space")]
    OutOfSymbolSpace,
    #[error("Memory misaligned")]
    MemoryMisaligned,
    #[error("Type Mismatch")]
    TypeMismatch,
    #[error("Stack overflow")]
    StackOverflow,
    #[error("Word not found: {0}")]
    WordNotFound(Symbol),
    #[error("Internal error")]
    InternalError,
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Function not found: {0}")]
    FunctionNotFound(u32),
    #[error(transparent)]
    RuntimeError(#[from] anyhow::Error),
    #[error(transparent)]
    ParseError(#[from] ParseError),
    #[error("Memory access error")]
    MemoryAccessError,
    #[error("BadArguments")]
    BadArguments,
}

#[derive(Debug)]
pub struct Memory<'a> {
    symbol_table: &'a mut [u32],
    symbol_count: usize,
    heap: &'a mut [u32],
    heap_ptr: usize,
    stack: &'a mut [u32],
    stack_ptr: usize,
    ops: &'a mut [u32],
    ops_ptr: usize,
    env: &'a mut [u32],
    env_ptr: usize,
    natives: Vec<NativeFn>,
}

impl<'a> Memory<'a> {
    const ENV_STACK_SIZE: usize = 128;
    const OP_STACK_SIZE: usize = 128;

    pub fn new(
        mem: &'a mut [u32],
        heap_start: usize,
        stack_size: usize,
    ) -> Result<Self, MemoryError> {
        let (symbol_table, rest) = mem.split_at_mut(heap_start);
        let (heap, rest) = rest.split_at_mut(rest.len() - stack_size);
        let (stack, rest) = rest.split_at_mut(Self::ENV_STACK_SIZE + Self::OP_STACK_SIZE);
        let (env, ops) = rest.split_at_mut(Self::ENV_STACK_SIZE);
        Ok(Self {
            symbol_table,
            heap,
            stack,
            ops,
            env,
            symbol_count: 0,
            heap_ptr: 1, //TODO: something, we should not use 0 addresse
            stack_ptr: 0,
            ops_ptr: 0,
            env_ptr: 0,
            natives: Vec::new(),
        })
    }

    // fn get_global(&self, global: usize) -> Option<u32> {
    //     self.globals.get(global).copied()
    // }

    // fn set_global(&mut self, global: usize, value: u32) -> Result<(), MemoryError> {
    //     self.globals
    //         .get_mut(global)
    //         .map(|slot| *slot = value)
    //         .ok_or(MemoryError::MemoryAccessError)
    // }

    fn get_top_env(&self) -> Option<Context> {
        self.env.last().copied().map(Context)
    }

    fn set_top_env(&mut self, ctx: Context) -> Option<()> {
        self.env.last_mut().map(|slot| *slot = ctx.0)
    }

    fn get_env(&self, symbol: Symbol) -> Option<Value> {
        self.env.get(0..self.env_ptr).and_then(|env| {
            env.iter()
                .rev()
                .find_map(|&ctx| Context(ctx).get(self, symbol))
        })
    }

    pub fn load_module(&mut self, module: &Module) -> Result<(), MemoryError> {
        let mut ctx = self.get_top_env().ok_or(MemoryError::MemoryAccessError)?;
        for (symbol, proc) in module.procs.iter() {
            let id = self.natives.len();
            self.natives.push(*proc);
            let native_fn = Value::native_fn(id as u32);
            let symbol = self.get_or_insert_symbol(symbol)?;
            ctx = ctx
                .add(self, symbol, native_fn)
                .ok_or(MemoryError::MemoryAccessError)?;
        }
        self.set_top_env(ctx).ok_or(MemoryError::MemoryAccessError)
    }

    fn call(&mut self, id: u32, bp: u32) -> Result<(), MemoryError> {
        if let Some(native_fn) = self.natives.get(id as usize) {
            native_fn(self, bp as usize)
        } else {
            Err(MemoryError::FunctionNotFound(id))
        }
    }

    fn encode_string(string: &str) -> [u32; 8] {
        let bytes = string.as_bytes();
        let len = bytes.len();
        if len < 32 {
            let mut buf = [0; 8];
            for i in 0..len {
                buf[i / 4] |= (bytes[i] as u32) << ((i % 4) * 8);
            }
            buf
        } else {
            unreachable!()
        }
    }

    fn decode_string(&self, address: Address) -> Result<&str, MemoryError> {
        let address = address as usize;
        let symbol_w = self
            .heap
            .get(address..address + 8)
            .ok_or(MemoryError::OutOfMemory)?;

        let symbol = unsafe { std::slice::from_raw_parts(symbol_w.as_ptr() as *const u8, 32) };

        for i in 0..32 {
            if symbol[i] == 0 {
                unsafe {
                    return Ok(std::str::from_utf8_unchecked(&symbol[..i]));
                }
            }
        }

        Err(MemoryError::StringTooLong)
    }

    #[allow(clippy::manual_memcpy)]
    fn alloc_encoded(&mut self, encoded: [u32; 8], len: usize) -> Result<Address, MemoryError> {
        assert!(len <= 8);
        if let Some(in_heap) = self.heap.get_mut(self.heap_ptr..self.heap_ptr + 8) {
            for i in 0..8 {
                in_heap[i] = encoded[i];
            }
            let address = self.heap_ptr as Address;
            self.heap_ptr += len;
            Ok(address)
        } else {
            Err(MemoryError::OutOfMemory)
        }
    }

    pub fn string(&mut self, string: &str) -> Result<Value, MemoryError> {
        let len = string.len();
        if len >= 32 {
            return Err(MemoryError::StringTooLong);
        }
        let encoded = Self::encode_string(string);
        self.alloc_encoded(encoded, len / 4 + 1)
            .map(|address| Value {
                tag: Value::STRING,
                value: address,
            })
    }

    pub fn as_str(&self, value: Value) -> Result<&str, MemoryError> {
        match value.tag {
            Value::STRING => self.decode_string(value.value),
            _ => Err(MemoryError::TypeMismatch),
        }
    }

    #[allow(clippy::manual_memcpy)]
    pub fn block(&mut self, stack_start: usize) -> Result<Block, MemoryError> {
        if let Some(in_stack) = self.stack.get(stack_start..self.stack_ptr) {
            let len = in_stack.len();
            if let Some(in_heap) = self.heap.get_mut(self.heap_ptr..self.heap_ptr + len + 1) {
                if let Some((hdr, payload)) = in_heap.split_first_mut() {
                    *hdr = (len / 2) as u32;
                    for i in 0..len {
                        payload[i] = in_stack[i];
                    }
                    let address = self.heap_ptr as Address;
                    self.heap_ptr += len + 1;
                    self.stack_ptr = stack_start;
                    Ok(Block(address))
                } else {
                    Err(MemoryError::OutOfMemory)
                }
            } else {
                Err(MemoryError::OutOfMemory)
            }
        } else {
            Err(MemoryError::StackOverflow)
        }
    }

    pub fn word(&mut self, symbol: &str) -> Result<Value, MemoryError> {
        Ok(Value {
            tag: Value::WORD,
            value: self.get_or_insert_symbol(symbol)?,
        })
    }

    pub fn set_word(&mut self, symbol: &str) -> Result<Value, MemoryError> {
        Ok(Value {
            tag: Value::SET_WORD,
            value: self.get_or_insert_symbol(symbol)?,
        })
    }

    pub fn get_or_insert_symbol(&mut self, symbol: &str) -> Result<Symbol, MemoryError> {
        let bytes = symbol.as_bytes();
        let len = bytes.len();
        if len >= 32 {
            return Err(MemoryError::StringTooLong);
        }
        let table_len = self.symbol_table.len();
        if table_len == 0 {
            return Err(MemoryError::OutOfSymbolSpace);
        }

        let encoded = Self::encode_string(symbol);
        let words = len / 4 + 1;
        let h = fast_hash(&encoded) as usize;

        let mut index = h % table_len;

        for _probe in 0..table_len {
            let stored_offset = self.symbol_table[index];

            if stored_offset == 0 {
                let address = self.alloc_encoded(encoded, words)?;
                self.symbol_table[index] = address;
                self.symbol_count += 1;
                return Ok(address);
            }

            let sym = stored_offset as usize;
            if let Some(existing) = self.heap.get(sym..sym + words) {
                let mut matches = true;
                for i in 0..words {
                    if existing[i] != encoded[i] {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    return Ok(stored_offset);
                }
            } else {
                return Err(MemoryError::OutOfMemory);
            }
            index = (index + 1) % table_len;
        }

        Err(MemoryError::OutOfSymbolSpace)
    }

    pub fn stack_pointer(&self) -> usize {
        self.stack_ptr
    }

    pub fn push(&mut self, value: Value) -> Option<()> {
        self.stack
            .get_mut(self.stack_ptr..self.stack_ptr + 2)
            .map(|slot| {
                slot[0] = value.tag;
                slot[1] = value.value;
                self.stack_ptr += 2;
            })
    }

    pub fn pop_from(&mut self, bp: usize) -> Option<&[u32]> {
        self.stack.get(bp..self.stack_ptr).inspect(|_| {
            self.stack_ptr = bp;
        })
    }

    pub fn peek(&mut self, bp: usize) -> Option<Value> {
        if bp + 2 > self.stack_ptr {
            None
        } else {
            self.stack.get(bp..bp + 2).map(|mem| Value {
                tag: mem[0],
                value: mem[1],
            })
        }
    }

    pub fn clear_stack(&mut self) -> Option<Value> {
        self.peek(0).inspect(|_| self.stack_ptr = 0)
    }

    fn push_op(&mut self, value: Value) -> Option<()> {
        self.ops
            .get_mut(self.ops_ptr..self.ops_ptr + 3)
            .map(|stack| {
                stack[0] = value.tag;
                stack[1] = value.value;
                stack[2] = self.stack_ptr as u32;
                self.ops_ptr += 3;
            })
    }

    fn pop_op(&mut self) -> Option<&[u32]> {
        if self.ops_ptr < 3 {
            None
        } else {
            self.ops_ptr -= 3;
            self.ops.get(self.ops_ptr..self.ops_ptr + 3)
        }
    }
}

//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_insert() -> Result<(), MemoryError> {
        let mut mem = vec![0; 0x10000];
        let mut layout = Memory::new(&mut mem, 0x1000, 0x1000)?;
        let symbol = layout.get_or_insert_symbol("hello")?;
        assert_eq!(symbol, 1);
        let symbol = layout.get_or_insert_symbol("hello")?;
        assert_eq!(symbol, 1);
        Ok(())
    }

    pub fn run(input: &str) -> Result<Value, MemoryError> {
        let mut bytes = vec![0; 0x10000];
        let mut memory = Memory::new(&mut bytes, 0x1000, 0x1000)?;
        memory.load_module(&crate::boot::CORE_MODULE)?;
        let block = Block::parse(&mut memory, input)?;
        let ctx = Context::empty();
        block.eval(&mut memory, ctx)
    }

    #[test]
    fn test_set_get_word() -> anyhow::Result<()> {
        let mut bytes = vec![0; 0x10000];
        let mut memory = Memory::new(&mut bytes, 0x1000, 0x1000)?;
        let mut process = Process::new(&mut memory);
        process.load_module(&crate::boot::CORE_MODULE)?;

        let value = process.eval("x: 5")?;
        assert_eq!(5 as i32, value.try_into()?);

        let value = process.eval("x")?;
        assert_eq!(5 as i32, value.try_into()?);

        let value = process.eval("add x 2")?;
        assert_eq!(7 as i32, value.try_into()?);

        Ok(())
    }

    #[test]
    fn test_read_all_1() -> anyhow::Result<()> {
        let value = run("5")?;
        assert_eq!(5 as i32, value.try_into()?);
        Ok(())
    }

    #[test]
    fn test_proc_1() -> anyhow::Result<()> {
        let value = run("add 7 8")?;
        assert_eq!(15 as i32, value.try_into()?);
        Ok(())
    }
}
