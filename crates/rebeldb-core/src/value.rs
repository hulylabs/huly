//

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValueError {
    #[error("String too long to be inlined")]
    StringTooLong,
    #[error("Type mismatch")]
    TypeMismatch,
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Bad range")]
    BadRange,
    #[error("Integer out of range for 47-bit payload")]
    IntegerOutOfRange,
}

type Tag = u8; // we use only 4 bits

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueType {
    Int = 0x0,
    // Bytes = 0x2,
    String = 0x3,
    Block = 0x4,
    Word = 0x5,
    NativeFn = 0x6,
    Context = 0x7,
    SetWord = 0x8,
    None = 0xf,
}

type PayloadHighBits = u16;

// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
// enum WordKind {
//     Word = 0,
//     SetWord = 1,
// }

type WasmWord = u32;
type Address = WasmWord;
type Symbol = WasmWord;

pub const HASH_SIZE: usize = 32;
pub type Hash = [u8; HASH_SIZE];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value(u64);

impl Value {
    // We force exponent=0x7FF => bits 62..52
    const EXP_SHIFT: u64 = 52;
    const EXP_MAX: u64 = 0x7FF;
    const EXP_MASK: u64 = Self::EXP_MAX << Self::EXP_SHIFT; // bits 62..52 = all ones

    // We'll always set fraction bit 51 = 1, so fraction != 0 => guaranteed NaN.
    const FRACTION_TOP_BIT: u64 = 1 << 51; // 0x8000_0000_0000

    // 4-bit tag in bits 50..47
    const TAG_BITS: u64 = 4;
    const TAG_SHIFT: u64 = 51 - Self::TAG_BITS;
    const TAG_MASK: u64 = 0xF;

    // That leaves bits 46..0 (47 bits) for the payload.
    const PAYLOAD_BITS: u64 = 51 - Self::TAG_BITS;
    const PAYLOAD_MASK: u64 = (1 << Self::PAYLOAD_BITS) - 1; // 0x7FFF_FFFF_FFFF

    // To allow either sign bit (bit 63) to be 0 or 1, we mask off everything
    // except exponent (bits 62..52) and the top fraction bit (bit 51).
    // We compare against the pattern indicating exponent=0x7FF and fraction’s top bit=1.
    const QNAN_MASK: u64 = 0x7FF8_0000_0000_0000;
    const TYPE_MASK: u64 = Self::QNAN_MASK | (Self::TAG_MASK << Self::TAG_SHIFT);

    pub const TAG_INT: Tag = ValueType::Int as Tag;
    pub const TAG_STRING: Tag = ValueType::String as Tag;
    pub const TAG_WORD: Tag = ValueType::Word as Tag;
    pub const TAG_NATIVE_FN: Tag = ValueType::NativeFn as Tag;

    pub fn none() -> Self {
        let fraction = Self::FRACTION_TOP_BIT | Self::tag_bits(ValueType::None);
        let bits = Self::EXP_MASK | fraction;
        Value(bits)
    }

    fn verify(&self, value_type: ValueType) -> Result<(), ValueError> {
        if self.bits() & Self::TYPE_MASK == Self::QNAN_MASK | (value_type as u64) << Self::TAG_SHIFT
        {
            Ok(())
        } else {
            Err(ValueError::TypeMismatch)
        }
    }

    fn tag_bits(tag: ValueType) -> u64 {
        ((tag as u64) & Self::TAG_MASK) << Self::TAG_SHIFT
    }

    pub fn tag(&self) -> Tag {
        ((self.0 >> Self::TAG_SHIFT) & Self::TAG_MASK) as Tag
    }

    pub fn new_int_unchecked(value: i64) -> Self {
        let payload_47 = ((value << (64 - Self::PAYLOAD_BITS)) >> (64 - Self::PAYLOAD_BITS)) as u64
            & Self::PAYLOAD_MASK;
        let fraction = Self::FRACTION_TOP_BIT | Self::tag_bits(ValueType::Int) | payload_47;
        let bits = Self::EXP_MASK | fraction;
        Value(bits)
    }

    /// Create a boxed *signed* integer with 47-bit 2's complement payload.
    ///
    /// Valid range: -2^46 .. 2^46 - 1
    /// (i.e. about ±140.7 trillion)
    pub fn new_int(value: i64) -> Result<Self, ValueError> {
        let min = -(1 << (Self::PAYLOAD_BITS - 1)); // -140,737,488,355,328
        let max = (1 << (Self::PAYLOAD_BITS)) - 1; // +140,737,488,355,327
        if value >= min && value <= max {
            Ok(Self::new_int_unchecked(value))
        } else {
            Err(ValueError::IntegerOutOfRange)
        }
    }

    pub fn as_int(&self) -> Result<i64, ValueError> {
        self.verify(ValueType::Int)?;
        let bits = self.bits();
        let payload = bits & Self::PAYLOAD_MASK;
        let shifted = (payload << (64 - Self::PAYLOAD_BITS)) as i64; // cast to i64 => preserve bits
        let value = shifted >> (64 - Self::PAYLOAD_BITS); // arithmetic shift right
        Ok(value)
    }

    pub fn native_fn(proc_id: WasmWord) -> Self {
        let payload = proc_id as u64;
        let fraction = Self::FRACTION_TOP_BIT
            | Self::tag_bits(ValueType::NativeFn)
            | (payload & Self::PAYLOAD_MASK);
        let bits = Self::EXP_MASK | fraction;
        Value(bits)
    }

    pub fn as_native_fn(&self) -> Result<WasmWord, ValueError> {
        self.verify(ValueType::NativeFn)?;
        Ok(self.wasm_word())
    }

    fn new_ptr(tag: ValueType, payload: PayloadHighBits, addr: Address) -> Self {
        let payload_47 = ((payload as u64) << 32) | ((addr as u64) & 0xFFFF_FFFF);
        let fraction =
            Self::FRACTION_TOP_BIT | Self::tag_bits(tag) | (payload_47 & Self::PAYLOAD_MASK);
        let bits = Self::EXP_MASK | fraction;
        Value(bits)
    }

    fn wasm_word(&self) -> WasmWord {
        (self.0 & 0xFFFF_FFFF) as WasmWord
    }

    fn address(&self) -> Address {
        self.wasm_word()
    }

    pub fn symbol(&self) -> Symbol {
        self.wasm_word()
    }

    fn payload_high_bits(&self) -> u16 {
        ((self.0 & Self::PAYLOAD_MASK) >> 32) as u16
    }

    /// Raw bits for debugging or advanced usage
    pub fn bits(&self) -> u64 {
        self.0
    }

    // fn write_value(mem: &mut impl Memory, addr: Address, value: Value) -> Result<(), ValueError> {
    //     Ok(mem
    //         .get_slice_mut(addr, std::mem::size_of::<Value>())?
    //         .copy_from_slice(&value.bits().to_le_bytes()))
    // }

    fn inline_string(mem: &mut impl Memory, string: &str) -> Result<WasmWord, ValueError> {
        let len = string.len();
        if len > HASH_SIZE {
            Err(ValueError::StringTooLong)
        } else {
            let (addr, dst) = mem.alloc(HASH_SIZE)?;
            let src = string.as_bytes();
            for i in 0..HASH_SIZE {
                dst[i] = if i < len { src[i] } else { 0 };
            }
            Ok(addr)
        }
    }

    pub fn string(mem: &mut impl Memory, string: &str) -> Result<Value, ValueError> {
        let len = string.len();
        if len <= HASH_SIZE {
            Ok(Value::new_ptr(
                ValueType::String,
                len as u16,
                Self::inline_string(mem, string)?,
            ))
        } else {
            Err(ValueError::StringTooLong)
        }
    }

    pub fn block(mem: &mut impl Memory, blk: &[Value]) -> Result<Value, ValueError> {
        let len = blk.len();
        let (addr, dst) = mem.alloc(len * 8)?;
        let value = Value::new_ptr(ValueType::Block, len as u16, addr);
        let mut offset = 0;
        for &v in blk {
            dst[offset..offset + 8].copy_from_slice(&v.bits().to_le_bytes());
            offset += 8;
        }
        Ok(value)
    }

    fn as_string_from_ptr(mem: &impl Memory, value: Value) -> &str {
        let len = value.payload_high_bits() as usize;
        let slice = mem.get_slice(value.address(), len);
        unsafe { std::str::from_utf8_unchecked(slice) }
    }

    pub fn context() -> Value {
        Value::new_ptr(ValueType::Context, 0, 0)
    }

    const CONTEXT_ENTRY_SIZE: usize = 16;

    /// Context entry layout:
    /// [0..4 4..8 8 .. 16]
    /// [symbol  prev  value]
    fn context_find(mem: &impl Memory, address: WasmWord, symbol: WasmWord) -> WasmWord {
        let mut address = address;
        while address != 0 {
            let hash = mem.get_slice(address, Self::CONTEXT_ENTRY_SIZE);
            let entry_symbol = u32::from_le_bytes(hash[0..4].try_into().unwrap());
            if entry_symbol == symbol {
                break;
            }
            address = u32::from_le_bytes(hash[4..8].try_into().unwrap());
        }
        address
    }

    pub fn context_get(
        mem: &impl Memory,
        addr: Address,
        symbol: Symbol,
    ) -> Result<Value, ValueError> {
        let entry_addr = Self::context_find(mem, addr, symbol);
        if entry_addr == 0 {
            Ok(Value::none())
        } else {
            let entry = mem.get_slice(entry_addr, Self::CONTEXT_ENTRY_SIZE);
            let value = u64::from_le_bytes(entry[8..16].try_into().unwrap());
            Ok(Value(value))
        }
    }

    pub fn context_put(
        mem: &mut impl Memory,
        addr: Address,
        symbol: Symbol,
        value: Value,
    ) -> Result<Value, ValueError> {
        let (entry_addr, entry) = mem.alloc(Self::CONTEXT_ENTRY_SIZE)?;
        entry[0..4].copy_from_slice(&symbol.to_le_bytes());
        entry[4..8].copy_from_slice(&addr.to_le_bytes());
        entry[8..16].copy_from_slice(&value.bits().to_le_bytes());
        let value = Value::new_ptr(ValueType::Context, 0, entry_addr);
        Ok(value)
    }

    pub fn word(mem: &mut impl Memory, string: &str) -> Result<Value, ValueError> {
        let addr = Self::inline_string(mem, string)?;
        let value = Value::new_ptr(ValueType::Word, 0, addr);
        Ok(value)
    }

    pub fn set_word(mem: &mut impl Memory, string: &str) -> Result<Value, ValueError> {
        let addr = Self::inline_string(mem, string)?;
        let value = Value::new_ptr(ValueType::SetWord, 0, addr);
        Ok(value)
    }

    pub fn as_inline_string(mem: &mut impl Memory, value: Value) -> Option<&str> {
        match value.tag() {
            Self::TAG_STRING => Some(Self::as_string_from_ptr(mem, value)),
            _ => None,
        }
    }
}

//

pub trait Memory {
    fn get_slice(&self, addr: WasmWord, len: usize) -> &[u8];
    fn get_slice_mut(&mut self, addr: WasmWord, len: usize) -> &mut [u8];
    fn alloc(&mut self, size: usize) -> Result<(Address, &mut [u8]), ValueError>;
    fn get_or_add_symbol(&mut self, symbol: &str) -> Result<Symbol, ValueError>;
}

pub struct OwnMemory {
    data: Vec<u8>,
    heap_ptr: usize,
    symbols: HashMap<Hash, Symbol>,
    symbols_ptr: usize,
}

impl OwnMemory {
    pub fn new(size: usize, symbols_start: usize, heap_start: usize) -> Self {
        Self {
            data: vec![0; size],
            heap_ptr: heap_start,
            symbols: HashMap::new(),
            symbols_ptr: symbols_start,
        }
    }
}

impl Memory for OwnMemory {
    fn get_slice(&self, addr: WasmWord, len: usize) -> &[u8] {
        let addr = addr as usize;
        &self.data[addr..addr + len]
    }

    fn get_slice_mut(&mut self, addr: WasmWord, len: usize) -> &mut [u8] {
        let addr = addr as usize;
        &mut self.data[addr..addr + len]
    }

    fn alloc(&mut self, size: usize) -> Result<(Address, &mut [u8]), ValueError> {
        let start = self.heap_ptr;
        let end = start + size;
        if end <= self.data.len() {
            self.heap_ptr += size;
            Ok((start as Address, &mut self.data[start..end]))
        } else {
            Err(ValueError::OutOfMemory)
        }
    }

    fn get_or_add_symbol(&mut self, symbol: &str) -> Result<Symbol, ValueError> {
        let mut bytes = [0u8; HASH_SIZE];
        bytes.copy_from_slice(symbol.as_bytes());
        if let Some(symbol) = self.symbols.get(&bytes) {
            Ok(*symbol)
        } else {
            let symbol = self.symbols_ptr as Symbol;
            self.data[self.symbols_ptr..self.symbols_ptr + HASH_SIZE].copy_from_slice(&bytes);
            self.symbols_ptr += HASH_SIZE;
            self.symbols.insert(bytes, symbol);
            Ok(symbol)
        }
    }
}

//

// #[inline(never)]
// pub fn string_test_1(memory: &mut Process<OwnMemory>, s: &str) -> Result<Value, ValueError> {
//     memory.string(s)
// }

// #[inline(never)]
// pub fn block_test_1(memory: &mut Process<OwnMemory>, blk: &[Value]) -> Result<Value, ValueError> {
//     memory.block(blk)
// }

// #[inline(never)]
// pub fn string_test_2(memory: &mut OwnedMemory, v: Value) -> Result<&str, ValueError> {
//     memory.as_string(v)
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_round_trip() -> Result<(), ValueError> {
        let vals = [
            0,
            1,
            -1,
            42,
            -42,
            123_456_789,
            -123_456_789,
            (1 << 46) - 1, //  140,737,488,355,327
            -(1 << 46),    // -140,737,488,355,328
        ];

        for &v in &vals {
            let b = Value::new_int(v)?;
            let back = b.as_int().unwrap();
            assert_eq!(
                v,
                back,
                "Failed round-trip for {} => bits=0x{:016X} => {}",
                v,
                b.bits(),
                back
            );
        }

        Ok(())
    }

    #[test]
    fn test_ptr_round_trip() {
        let ptrs = [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF];
        for &p in &ptrs {
            let b = Value::new_ptr(ValueType::String, 0, p);
            let back = b.address();
            assert_eq!(
                p,
                back,
                "Failed round-trip for pointer 0x{:08X} => bits=0x{:016X} => 0x{:08X}",
                p,
                b.bits(),
                back
            );
        }
    }

    #[test]
    fn test_string_1() -> Result<(), ValueError> {
        let mut mem = OwnMemory::new(0x10000, 0x100, 0x1000);

        let value = Value::string(&mut mem, "hello, world!")?;
        assert_eq!(
            Value::as_inline_string(&mut mem, value),
            Some("hello, world!")
        );
        Ok(())
    }
}
