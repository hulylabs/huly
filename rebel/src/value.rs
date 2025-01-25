// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use thiserror::Error;

pub type Address = u32;
pub type Symbol = Address;

pub enum Value {
    Int(u32),
    String(Address),
    Word(Symbol),
    SetWord(Symbol),
    None,
}

const TAG_INT: u8 = 0x0;
const TAG_STRING: u8 = 0x1;
const TAG_WORD: u8 = 0x2;
const TAG_SETWORD: u8 = 0x3;
const TAG_NONE: u8 = 0x4;

fn tag(tag: u8, value: u32) -> u64 {
    value as u64 | (tag as u64) << 32
}

impl Into<u64> for Value {
    fn into(self) -> u64 {
        match self {
            Value::Int(i) => tag(TAG_INT, i),
            Value::String(a) => tag(TAG_STRING, a),
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
            TAG_WORD => Value::Word(value),
            TAG_SETWORD => Value::SetWord(value),
            TAG_NONE => Value::None,
            _ => unreachable!(),
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
}

type Hash = [u8; 32];

pub struct MemoryLayout<'a> {
    symbol_table: &'a mut [u32],
    symbol_count: usize,
    heap: &'a mut [u8],
    heap_ptr: usize,
}

impl<'a> MemoryLayout<'a> {
    const MIN_TABLE_SIZE: usize = 16;

    pub fn new(
        mem: &'a mut [u8],
        symbol_start: usize,
        heap_start: usize,
    ) -> Result<Self, MemoryError> {
        let table_bytes = heap_start
            .checked_sub(symbol_start)
            .ok_or(MemoryError::OutOfSymbolSpace)?;

        if table_bytes < Self::MIN_TABLE_SIZE * 4 {
            return Err(MemoryError::OutOfSymbolSpace);
        }

        if (table_bytes % 4) != 0 {
            return Err(MemoryError::SymbolTooLong);
        }

        // Split memory
        let (pre_table, rest) = mem.split_at_mut(heap_start);
        let (_, table_bytes) = pre_table.split_at_mut(symbol_start);

        // SAFETY:
        // 1. We verified table_bytes.len() >= MIN_TABLE_SIZE * 4
        // 2. We verified table_bytes.len() is multiple of 4
        let symbol_table = unsafe {
            std::slice::from_raw_parts_mut(
                table_bytes.as_mut_ptr() as *mut u32,
                table_bytes.len() / 4,
            )
        };

        Ok(Self {
            symbol_table,
            heap: rest,
            symbol_count: 0,
            heap_ptr: 4,
        })
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

    fn get_or_insert(&mut self, symbol: &Hash) -> Result<Symbol, MemoryError> {
        let h = Self::fast_hash_32(symbol) as usize;
        let table_len = self.symbol_table.len();
        if table_len == 0 {
            return Err(MemoryError::OutOfSymbolSpace);
        }
        let mut index = h % table_len;

        for _probe in 0..table_len {
            let stored_offset = self.symbol_table[index];

            if stored_offset == 0 {
                let symbol_offset = self.alloc_symbol(symbol)?;
                self.symbol_count += 1;
                self.symbol_table[index] = symbol_offset;
                return Ok(symbol_offset);
            }

            let sym = stored_offset as usize;
            if let Some(existing_sym) = self.heap.get(sym..sym + 32) {
                if existing_sym == symbol {
                    return Ok(stored_offset);
                }
            } else {
                return Err(MemoryError::OutOfMemory);
            }

            index = (index + 1) % table_len;
        }

        Err(MemoryError::OutOfSymbolSpace)
    }
}

//

#[inline(never)]
pub fn get_or_insert(layout: &mut MemoryLayout, symbol: &Hash) -> Value {
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
        let mut layout = MemoryLayout::new(&mut mem, 0x1000, 0x2000)?;
        let mut hash: Hash = [0; 32];
        hash[..5].copy_from_slice(b"hello");
        let symbol = layout.get_or_insert(&hash)?;
        assert_eq!(symbol, 4);
        let symbol = layout.get_or_insert(&hash)?;
        assert_eq!(symbol, 4);
        Ok(())
    }
}
