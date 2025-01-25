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
    const INT: u32 = 0x0;
    const STRING: u32 = 0x1;
    const BLOCK: u32 = 0x2;
    const CONTEXT: u32 = 0x3;
    const WORD: u32 = 0x4;
    const SET_WORD: u32 = 0x5;
    const NATIVE_FN: u32 = 0x6;
    const NONE: u32 = 0x7;

    pub fn from_i32(value: i32) -> Self {
        let value = value as u32;
        Value {
            tag: Self::INT,
            value,
        }
    }

    pub fn try_into_i32(&self) -> Option<i32> {
        match self.tag {
            Self::INT => Some(self.value as i32),
            _ => None,
        }
    }

    pub fn none() -> Self {
        Value {
            tag: Self::NONE,
            value: 0,
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

    // pub fn add(
    //     &self,
    //     memory: &mut Memory,
    //     symbol: Symbol,
    //     value: Value,
    // ) -> Result<Self, MemoryError> {
    //     let mut addr = self.0;
    //     while addr != 0 {
    //         let sym = memory.get_symbol(addr)?;
    //         if sym == symbol {
    //             return Err(MemoryError::OutOfMemory);
    //         }
    //         addr = memory.get_next(addr)?;
    //     }
    // }
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
            stack_ptr: stack_size,
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
        let mut w = 0;
        let len = loop {
            let val = self.heap[address + w];
            w += 1;
            let zero_bytes = (val.leading_zeros() / 8) as usize;
            if zero_bytes > 0 {
                break w * 4 - zero_bytes;
            }
        };
        unsafe {
            Ok(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                self.heap.as_ptr().add(address) as *const u8,
                len,
            )))
        }
    }

    fn alloc_encoded(&mut self, encoded: [u32; 8], len: usize) -> Result<Address, MemoryError> {
        assert!(len <= 8);
        if self.heap_ptr + 8 > self.heap.len() {
            return Err(MemoryError::OutOfMemory);
        }
        let address = self.heap_ptr as Address;
        unsafe {
            let dst = self.heap.as_mut_ptr().add(self.heap_ptr) as *mut [u32; 8];
            core::ptr::write(dst, encoded);
        }
        self.heap_ptr += len;
        Ok(address)
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

    pub fn block(&mut self, stack_start: usize) -> Result<Value, MemoryError> {
        let len = stack_start
            .checked_sub(self.stack_ptr)
            .ok_or(MemoryError::StackOverflow)?;

        if stack_start > self.stack.len() {
            return Err(MemoryError::StackOverflow);
        }

        let new_ptr = self
            .heap_ptr
            .checked_add(len + 1)
            .filter(|&ptr| ptr <= self.heap.len())
            .ok_or(MemoryError::OutOfMemory)?;

        self.heap[self.heap_ptr] = (len / 2) as u32;
        for i in 1..len + 1 {
            self.heap[self.heap_ptr + i] = self.stack[stack_start - i];
        }

        let address = self.heap_ptr as Address;
        self.heap_ptr = new_ptr;
        self.stack_ptr = stack_start;
        Ok(Value {
            tag: Value::BLOCK,
            value: address,
        })
    }

    pub fn word(&mut self, symbol: &str) -> Result<Value, MemoryError> {
        Ok(Value {
            tag: Value::WORD,
            value: self.get_or_insert(symbol)?,
        })
    }

    pub fn set_word(&mut self, symbol: &str) -> Result<Value, MemoryError> {
        Ok(Value {
            tag: Value::SET_WORD,
            value: self.get_or_insert(symbol)?,
        })
    }

    fn get_or_insert(&mut self, symbol: &str) -> Result<Symbol, MemoryError> {
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
        if self.stack_ptr < 2 {
            return Err(MemoryError::StackOverflow);
        }
        self.stack_ptr -= 2;
        self.stack[self.stack_ptr] = value.tag;
        self.stack[self.stack_ptr + 1] = value.value;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Value, MemoryError> {
        if self.stack_ptr + 2 > self.stack.len() {
            return Err(MemoryError::StackOverflow);
        }
        self.stack_ptr += 2;
        Ok(Value {
            tag: self.stack[self.stack_ptr - 2],
            value: self.stack[self.stack_ptr - 1],
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
        let symbol = layout.get_or_insert("hello")?;
        assert_eq!(symbol, 1);
        let symbol = layout.get_or_insert("hello")?;
        assert_eq!(symbol, 1);
        Ok(())
    }
}
