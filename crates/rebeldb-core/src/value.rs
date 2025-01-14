//

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValueError {
    #[error("Not a RebelDB value")]
    NotAValue,
    #[error("String too long")]
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

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueType {
    Int = 0x0,
    // Float = 0x1,
    // Bytes = 0x2,
    String = 0x3,
    Block = 0x4,
    Word = 0x5,
    None = 0xf,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WordKind {
    Word = 0,
    SetWord = 1,
}

pub type WasmWord = u32;

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
    const TAG_SHIFT: u64 = 47;
    const TAG_MASK: u64 = 0xF;

    // That leaves bits 46..0 (47 bits) for the payload.
    const PAYLOAD_MASK_47: u64 = (1 << 47) - 1; // 0x7FFF_FFFF_FFFF

    // To allow either sign bit (bit 63) to be 0 or 1, we mask off everything
    // except exponent (bits 62..52) and the top fraction bit (bit 51).
    // We compare against the pattern indicating exponent=0x7FF and fraction’s top bit=1.
    const QNAN_MASK: u64 = 0x7FF8_0000_0000_0000;

    const HASH_SIZE: usize = 32;

    pub fn none() -> Self {
        let fraction = Self::FRACTION_TOP_BIT | Self::tag_bits(ValueType::None);
        let bits = (0 << 63) | Self::EXP_MASK | fraction;
        Value(bits)
    }

    pub fn verify(bits: u64) -> Result<(), ValueError> {
        if (bits & Self::QNAN_MASK) == Self::QNAN_MASK {
            Ok(())
        } else {
            Err(ValueError::NotAValue)
        }
    }

    fn tag_bits(tag: ValueType) -> u64 {
        ((tag as u64) & Self::TAG_MASK) << Self::TAG_SHIFT
    }

    pub fn tag(&self) -> u64 {
        let bits = self.0;
        let fraction = bits & ((1 << 52) - 1); // lower 52 bits
        (fraction >> Self::TAG_SHIFT) & Self::TAG_MASK
    }

    pub fn new_int_unchecked(value: i64) -> Self {
        let payload_47 = ((value << (64 - 47)) >> (64 - 47)) as u64 & Self::PAYLOAD_MASK_47;
        let fraction = Self::FRACTION_TOP_BIT | Self::tag_bits(ValueType::Int) | payload_47;
        let bits = (0 << 63) | Self::EXP_MASK | fraction;
        Value(bits)
    }

    /// Create a boxed *signed* integer with 47-bit 2's complement payload.
    ///
    /// Valid range: -2^46 .. 2^46 - 1
    /// (i.e. about ±140.7 trillion)
    pub fn new_int(value: i64) -> Result<Self, ValueError> {
        let min = -(1 << 46); // -140,737,488,355,328
        let max = (1 << 46) - 1; // +140,737,488,355,327
        if value >= min && value <= max {
            Ok(Self::new_int_unchecked(value))
        } else {
            Err(ValueError::IntegerOutOfRange)
        }
    }

    /// Interpret this Value as a 47-bit signed integer.
    pub fn as_int(&self) -> Option<i64> {
        if self.tag() == ValueType::Int as u64 {
            let bits = self.0;
            let payload_47 = bits & Self::PAYLOAD_MASK_47;
            let shifted = (payload_47 << (64 - 47)) as i64; // cast to i64 => preserve bits
            let value = shifted >> (64 - 47); // arithmetic shift right
            Some(value)
        } else {
            None
        }
        // let bits = self.0;
        // let payload_47 = bits & PAYLOAD_MASK_47;
        // let shifted = (payload_47 << (64 - 47)) as i64; // cast to i64 => preserve bits
        // let value = shifted >> (64 - 47); // arithmetic shift right
        // value
    }

    /// Create a boxed pointer (32 bits). Tag = Ptr, fraction bit 51=1, payload in bits 46..0.
    fn new_ptr(tag: ValueType, payload: u16, addr: WasmWord) -> Self {
        let payload_47 = ((payload as u64) << 32) | ((addr as u64) & 0xFFFF_FFFF);
        let fraction =
            Self::FRACTION_TOP_BIT | Self::tag_bits(tag) | (payload_47 & Self::PAYLOAD_MASK_47);
        let bits = (0 << 63) | Self::EXP_MASK | fraction;
        Value(bits)
    }

    /// Return the pointer as 32 bits.
    fn address(&self) -> WasmWord {
        (self.0 & 0xFFFF_FFFF) as WasmWord
    }

    fn payload_high_bits(&self) -> u16 {
        ((self.0 & Self::PAYLOAD_MASK_47) >> 32) as u16
    }

    /// Raw bits for debugging or advanced usage
    pub fn bits(&self) -> u64 {
        self.0
    }

    fn write_value(mem: &mut impl Memory, value: Value) -> Result<(), ValueError> {
        let dst = mem.alloc(std::mem::size_of::<Value>())?;
        dst.copy_from_slice(&value.bits().to_le_bytes());
        Ok(())
    }

    fn inline_string(mem: &mut impl Memory, string: &str) -> Result<WasmWord, ValueError> {
        let len = string.len();
        if len > Self::HASH_SIZE {
            Err(ValueError::StringTooLong)
        } else {
            let addr = mem.heap_end();
            let dst = mem.alloc(Self::HASH_SIZE)?;
            let src = string.as_bytes();
            for i in 0..Self::HASH_SIZE {
                dst[i] = if i < len { src[i] } else { 0 };
            }
            Ok(addr)
        }
    }

    pub fn string(mem: &mut impl Memory, string: &str) -> Result<Value, ValueError> {
        let len = string.len();
        if len <= Self::HASH_SIZE {
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
        let addr = mem.heap_end();
        let value = Value::new_ptr(ValueType::Block, len as u16, addr);
        for &v in blk {
            Self::write_value(mem, v)?;
        }
        Ok(value)
    }

    pub fn word(mem: &mut impl Memory, string: &str) -> Result<Value, ValueError> {
        let addr = Self::inline_string(mem, string)?;
        let value = Value::new_ptr(ValueType::Word, WordKind::Word as u16, addr);
        Ok(value)
    }

    pub fn set_word(mem: &mut impl Memory, string: &str) -> Result<Value, ValueError> {
        let addr = Self::inline_string(mem, string)?;
        let value = Value::new_ptr(ValueType::Word, WordKind::SetWord as u16, addr);
        Ok(value)
    }

    fn as_string_from_ptr(mem: &mut impl Memory, value: Value) -> Result<&str, ValueError> {
        let addr = value.address() as usize;
        let len = value.payload_high_bits() as usize;
        let slice = mem.get_slice(addr, len)?;
        unsafe { Ok(std::str::from_utf8_unchecked(slice)) }
    }

    pub fn as_inline_string(
        mem: &mut impl Memory,
        value: Value,
    ) -> Result<Option<&str>, ValueError> {
        const STRING_TYPE: u64 = ValueType::String as u64;
        match value.tag() {
            STRING_TYPE => Some(Self::as_string_from_ptr(mem, value)).transpose(),
            _ => Err(ValueError::TypeMismatch),
        }
    }
}

//

pub trait Memory {
    fn get_slice(&self, addr: usize, len: usize) -> Result<&[u8], ValueError>;
    // fn get_mut_slice(&mut self, range: Range<usize>) -> Result<&mut [u8], ValueError>;
    fn alloc(&mut self, size: usize) -> Result<&mut [u8], ValueError>;

    fn heap_end(&self) -> WasmWord;
    fn size(&self) -> WasmWord;
}

pub struct OwnMemory {
    heap_end: usize,
    data: Vec<u8>,
}

impl OwnMemory {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
            heap_end: 0,
        }
    }
}

impl Memory for OwnMemory {
    fn heap_end(&self) -> WasmWord {
        self.heap_end as WasmWord
    }

    fn get_slice(&self, addr: usize, len: usize) -> Result<&[u8], ValueError> {
        if addr + len <= self.data.len() {
            Ok(&self.data[addr..addr + len])
        } else {
            Err(ValueError::BadRange)
        }
    }

    fn alloc(&mut self, size: usize) -> Result<&mut [u8], ValueError> {
        let addr = self.heap_end;
        if addr + size <= self.data.len() {
            self.heap_end += size;
            Ok(&mut self.data[addr..addr + size])
        } else {
            Err(ValueError::OutOfMemory)
        }
    }

    // fn get_mut_slice(&mut self, range: Range<usize>) -> Result<&mut [u8], ValueError> {
    //     if range.end <= self.data.len() {
    //         if range.start <= range.end {
    //             Ok(&mut self.data[range])
    //         } else {
    //             Err(ValueError::BadRange)
    //         }
    //     } else {
    //         Err(ValueError::OutOfMemory)
    //     }
    // }

    fn size(&self) -> WasmWord {
        self.data.len() as WasmWord
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
        let mut mem = OwnMemory::new(65536);

        let value = Value::string(&mut mem, "hello, world!")?;
        assert_eq!(
            Value::as_inline_string(&mut mem, value)?,
            Some("hello, world!")
        );
        Ok(())
    }
}
