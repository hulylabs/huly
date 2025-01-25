// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::hash::fast_hash;
use thiserror::Error;

pub type Address = u32;
pub type Symbol = Address;

#[derive(Debug, Clone, Copy)]
pub struct Context(Address);

#[derive(Debug)]
pub struct Block(Address);

#[derive(Debug, Clone, Copy)]
pub struct Value {
    tag: u32,
    value: u32,
}

impl Value {
    pub const INT: u32 = 0x0;
    const STRING: u32 = 0x1;
    const BLOCK: u32 = 0x2;
    const CONTEXT: u32 = 0x3;
    pub const WORD: u32 = 0x4;
    const SET_WORD: u32 = 0x5;
    pub const NATIVE_FN: u32 = 0x6;
    const TAG_NONE: u32 = 0x7;

    pub const NONE: Value = Value {
        tag: Self::TAG_NONE,
        value: 0,
    };

    pub fn tag(&self) -> u32 {
        self.tag
    }

    pub fn payload(&self) -> u32 {
        self.value
    }

    pub fn native_fn(id: u32) -> Self {
        Value {
            tag: Self::NATIVE_FN,
            value: id,
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

impl Block {
    pub fn len(&self, memory: &Memory) -> usize {
        memory.heap[self.0 as usize] as usize
    }

    pub fn get(&self, memory: &Memory, index: usize) -> Value {
        let addr = self.0 as usize + index * 2 + 1;
        Value {
            tag: memory.heap[addr],
            value: memory.heap[addr + 1],
        }
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

impl Context {
    pub fn empty() -> Self {
        Context(0)
    }

    /// Each context entry stored as below (u32 values):
    /// [symbol, next, tag, value]
    pub fn add(
        self,
        memory: &mut Memory,
        symbol: Symbol,
        value: Value,
    ) -> Result<Self, MemoryError> {
        let mut addr = self.0;
        while addr != 0 {
            if let Some(entry) = memory.heap.get_mut(addr as usize..addr as usize + 4) {
                if entry[0] == symbol {
                    entry[2] = value.tag;
                    entry[3] = value.value;
                    return Ok(self);
                }
                addr = entry[1];
            } else {
                return Err(MemoryError::OutOfMemory);
            }
        }
        if let Some(new_entry) = memory.heap.get_mut(memory.heap_ptr..memory.heap_ptr + 4) {
            new_entry[0] = symbol;
            new_entry[1] = self.0;
            new_entry[2] = value.tag;
            new_entry[3] = value.value;
            let address = memory.heap_ptr as Address;
            memory.heap_ptr += 4;
            Ok(Context(address))
        } else {
            Err(MemoryError::OutOfMemory)
        }
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
}

#[derive(Debug)]
pub struct Memory<'a> {
    symbol_table: &'a mut [u32],
    symbol_count: usize,
    heap: &'a mut [u32],
    heap_ptr: usize,
    stack: &'a mut [u32],
    stack_ptr: usize,
}

impl<'a> Memory<'a> {
    pub fn new(
        mem: &'a mut [u32],
        heap_start: usize,
        stack_size: usize,
    ) -> Result<Self, MemoryError> {
        let (symbol_table, rest) = mem.split_at_mut(heap_start);
        let (heap, stack) = rest.split_at_mut(rest.len() - stack_size);
        Ok(Self {
            symbol_table,
            heap,
            stack,
            symbol_count: 0,
            heap_ptr: 1, //TODO: something, we should not use 0 addresse
            stack_ptr: 0,
        })
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

    pub fn as_str(&self, value: &Value) -> Result<&str, MemoryError> {
        match value.tag {
            Value::STRING => self.decode_string(value.value),
            _ => Err(MemoryError::TypeMismatch),
        }
    }

    #[allow(clippy::manual_memcpy)]
    pub fn block(&mut self, stack_start: usize) -> Result<Value, MemoryError> {
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
                    Ok(Value {
                        tag: Value::BLOCK,
                        value: address,
                    })
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

    pub fn push(&mut self, value: Value) -> Result<(), MemoryError> {
        if let Some(slot) = self.stack.get_mut(self.stack_ptr..self.stack_ptr + 2) {
            slot[0] = value.tag;
            slot[1] = value.value;
            self.stack_ptr += 2;
            Ok(())
        } else {
            Err(MemoryError::StackOverflow)
        }
    }

    pub fn pop_frame(&mut self, size: usize) -> Option<&[u32]> {
        self.stack_ptr.checked_sub(size * 2).and_then(|new_ptr| {
            self.stack.get(new_ptr..self.stack_ptr).inspect(|_| {
                self.stack_ptr = new_ptr;
            })
        })
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.pop_frame(1).map(|frame| Value {
            tag: frame[0],
            value: frame[1],
        })
    }
}

//

#[inline(never)]
pub fn make_word(layout: &mut Memory, symbol: &str) -> Result<Value, MemoryError> {
    layout.word(symbol)
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
}
