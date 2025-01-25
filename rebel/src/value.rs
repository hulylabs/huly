// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use thiserror::Error;

pub type Address = u32;
pub type Symbol = Address;

pub enum Value {
    Int(u32),
    String(Address),
    Block(Address),
    Word(Symbol),
    SetWord(Symbol),
    None,
}

const TAG_INT: u8 = 0x0;
const TAG_STRING: u8 = 0x1;
const TAG_BLOCK: u8 = 0x2;
const TAG_WORD: u8 = 0x3;
const TAG_SETWORD: u8 = 0x4;
const TAG_NONE: u8 = 0x5;

fn tag(tag: u8, value: u32) -> u64 {
    value as u64 | (tag as u64) << 32
}

impl Value {
    pub fn new_int(value: i32) -> Self {
        Value::Int(value as u32)
    }

    pub fn get_int(&self) -> Option<i32> {
        match *self {
            Value::Int(i) => Some(i as i32),
            _ => None,
        }
    }

    fn as_u64(&self) -> u64 {
        match *self {
            Value::Int(i) => tag(TAG_INT, i),
            Value::String(a) => tag(TAG_STRING, a),
            Value::Block(a) => tag(TAG_BLOCK, a),
            Value::Word(s) => tag(TAG_WORD, s),
            Value::SetWord(s) => tag(TAG_SETWORD, s),
            Value::None => tag(TAG_NONE, 0),
        }
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        let tag = (value >> 32) as u8;
        let value = value as u32;
        match tag {
            TAG_INT => Value::Int(value),
            TAG_STRING => Value::String(value),
            TAG_BLOCK => Value::Block(value),
            TAG_WORD => Value::Word(value),
            TAG_SETWORD => Value::SetWord(value),
            _ => Value::None,
        }
    }
}

//

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Symbol too long")]
    SymbolTooLong,
    #[error("Out of symbol space")]
    OutOfSymbolSpace,
    #[error("Memory misaligned")]
    MemoryMisaligned,
    #[error("Type Mismatch")]
    TypeMismatch,
    #[error("Stack overflow")]
    StackOverflow,
}

type Hash = [u8; 32];

pub struct Memory<'a> {
    symbol_table: &'a mut [u32],
    symbol_count: usize,
    heap: &'a mut [u8],
    heap_ptr: usize,
    stack: &'a mut [u64],
    stack_ptr: usize,
}

impl<'a> Memory<'a> {
    const MIN_TABLE_SIZE: usize = 16;

    pub fn new(
        mem: &'a mut [u8],
        symbol_start: usize,
        heap_start: usize,
        stack_size: usize,
    ) -> Result<Self, MemoryError> {
        let table_bytes = heap_start
            .checked_sub(symbol_start)
            .ok_or(MemoryError::OutOfSymbolSpace)?;

        if table_bytes < Self::MIN_TABLE_SIZE * 4 {
            return Err(MemoryError::OutOfSymbolSpace);
        }

        let (pre_table, rest) = mem.split_at_mut(heap_start);
        let (_, table_bytes) = pre_table.split_at_mut(symbol_start);
        let (heap, stack) = rest.split_at_mut(rest.len() - stack_size * 8);

        let symbol_table = unsafe {
            std::slice::from_raw_parts_mut(
                table_bytes.as_mut_ptr() as *mut u32,
                table_bytes.len() / 4,
            )
        };
        let stack = unsafe {
            std::slice::from_raw_parts_mut(stack.as_mut_ptr() as *mut u64, stack.len() / 8)
        };

        Ok(Self {
            symbol_table,
            heap,
            stack,
            symbol_count: 0,
            heap_ptr: 4, //TODO: something, we should not use 0 addresse
            stack_ptr: stack_size,
        })
    }

    pub fn new_string(&mut self, string: &str) -> Result<Value, MemoryError> {
        let len = string.len();
        if len > 0xffff {
            return Err(MemoryError::OutOfMemory);
        }

        let new_ptr = self
            .heap_ptr
            .checked_add(len + 2)
            .filter(|&ptr| ptr <= self.heap.len())
            .ok_or(MemoryError::OutOfMemory)?;

        // After bounds check, use unchecked operations
        unsafe {
            *self.heap.get_unchecked_mut(self.heap_ptr) = len as u8;
            *self.heap.get_unchecked_mut(self.heap_ptr + 1) = (len >> 8) as u8;

            std::ptr::copy_nonoverlapping(
                string.as_ptr(),
                self.heap.as_mut_ptr().add(self.heap_ptr + 2),
                len,
            );
        }

        let address = self.heap_ptr as Address;
        self.heap_ptr = new_ptr;
        Ok(Value::String(address))
    }

    pub fn get_string(&self, string: &Value) -> Result<&str, MemoryError> {
        match string {
            Value::String(address) => {
                let address = *address as usize;
                if address + 2 > self.heap.len() {
                    return Err(MemoryError::OutOfMemory);
                }
                let len = self.heap[address] as usize | ((self.heap[address + 1] as usize) << 8);
                unsafe {
                    Ok(std::str::from_utf8_unchecked(
                        self.heap
                            .get(address + 2..address + 2 + len)
                            .ok_or(MemoryError::OutOfMemory)?,
                    ))
                }
            }
            _ => Err(MemoryError::TypeMismatch),
        }
    }

    pub fn block(&mut self, stack_start: usize) -> Result<Value, MemoryError> {
        let len = stack_start
            .checked_sub(self.stack_ptr)
            .ok_or(MemoryError::StackOverflow)?;

        let new_ptr = self
            .heap_ptr
            .checked_add(len * 8)
            .filter(|&ptr| ptr <= self.heap.len())
            .ok_or(MemoryError::OutOfMemory)?;

        unsafe {
            for i in 0..len {
                let value = *self.stack.get_unchecked(self.stack_ptr + i);
                let dest = self.heap.as_mut_ptr().add(self.heap_ptr + i * 8) as *mut u64;
                *dest = value;
            }
        }

        let address = self.heap_ptr as Address;
        self.heap_ptr = new_ptr;
        self.stack_ptr = stack_start;
        Ok(Value::Block(address))
    }

    pub fn word(&mut self, symbol: &str) -> Result<Value, MemoryError> {
        Ok(Value::Word(self.get_or_insert(symbol)?))
    }

    pub fn set_word(&mut self, symbol: &str) -> Result<Value, MemoryError> {
        Ok(Value::SetWord(self.get_or_insert(symbol)?))
    }

    fn fast_hash_64(data: &[u8; 32]) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        // Transmute the 32 bytes into four 64-bit words.
        // SAFETY: We know `data` is exactly 32 bytes, so this is valid:
        let words = unsafe { &*(data as *const [u8; 32] as *const [u64; 4]) };

        let mut hash = FNV_OFFSET_BASIS;
        for &w in words {
            hash ^= w;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    fn fast_hash_32(data: &[u8; 32]) -> u32 {
        let h64 = Self::fast_hash_64(data);
        ((h64 >> 32) as u32) ^ (h64 as u32)
    }

    fn alloc_symbol(&mut self, hash: &Hash) -> Result<Symbol, MemoryError> {
        let new_ptr = self
            .heap_ptr
            .checked_add(32)
            .filter(|&ptr| ptr <= self.heap.len())
            .ok_or(MemoryError::OutOfMemory)?;

        self.heap
            .get_mut(self.heap_ptr..new_ptr)
            .expect("bounds already checked")
            .copy_from_slice(hash);

        let symbol = self.heap_ptr as Symbol;
        self.heap_ptr = new_ptr;
        Ok(symbol)
    }

    fn get_or_insert(&mut self, symbol: &str) -> Result<Symbol, MemoryError> {
        let len = symbol.len();
        if len >= 32 {
            return Err(MemoryError::SymbolTooLong);
        }

        let mut hash = [0; 32];
        hash[..len].copy_from_slice(symbol.as_bytes());

        let h = Self::fast_hash_32(&hash) as usize;
        let table_len = self.symbol_table.len();
        if table_len == 0 {
            return Err(MemoryError::OutOfSymbolSpace);
        }
        let mut index = h % table_len;

        for _probe in 0..table_len {
            let stored_offset = self.symbol_table[index];

            if stored_offset == 0 {
                let symbol_offset = self.alloc_symbol(&hash)?;
                self.symbol_count += 1;
                self.symbol_table[index] = symbol_offset;
                return Ok(symbol_offset);
            }

            let sym = stored_offset as usize;
            if let Some(existing_sym) = self.heap.get(sym..sym + 32) {
                if existing_sym == hash {
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
        if self.stack_ptr == 0 {
            return Err(MemoryError::StackOverflow);
        }
        self.stack_ptr -= 1;
        unsafe {
            *self.stack.get_unchecked_mut(self.stack_ptr) = value.as_u64();
        }
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Value, MemoryError> {
        if self.stack_ptr >= self.stack.len() {
            return Err(MemoryError::StackOverflow);
        }
        let value = unsafe { *self.stack.get_unchecked(self.stack_ptr) };
        self.stack_ptr += 1;
        Ok(Value::from(value))
    }
}

//

#[inline(never)]
pub fn get_or_insert(layout: &mut Memory, symbol: &str) -> Value {
    match layout.get_or_insert(symbol) {
        Ok(symbol) => Value::Word(symbol),
        Err(_) => Value::None,
    }
}

//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_or_insert() -> Result<(), MemoryError> {
        let mut mem = vec![0; 0x10000];
        let mut layout = Memory::new(&mut mem, 0x1000, 0x2000, 0x1000)?;
        let symbol = layout.get_or_insert("hello")?;
        assert_eq!(symbol, 4);
        let symbol = layout.get_or_insert("hello")?;
        assert_eq!(symbol, 4);
        Ok(())
    }
}
