//

use std::ops::Range;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValueError {
    #[error("Integer out of 47-bit range")]
    IntOutOfRange,
    #[error("Not a RebelDB value")]
    NotAValue,
    #[error("String too long")]
    StringTooLong,
    #[error("Type mismatch")]
    TypeMismatch,
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Access violation")]
    AccessViolation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value(u64);

// We force exponent=0x7FF => bits 62..52
const EXP_SHIFT: u64 = 52;
const EXP_MAX: u64 = 0x7FF;
const EXP_MASK: u64 = EXP_MAX << EXP_SHIFT; // bits 62..52 = all ones

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

//

const HASH_SIZE: usize = 32;

// trait Storage {
//     fn put(&mut self, data: &[u8]) -> Hash;
// }

// struct NullStorage;

// impl Storage for NullStorage {
//     fn put(&mut self, _data: &[u8]) -> Hash {
//         unreachable!()
//     }
// }

// trait Memory {
//     type Storage: Storage;
// }

pub trait Memory {
    fn get_slice(&self, range: Range<usize>) -> Result<&[u8], ValueError>;
    fn get_mut_slice(&mut self, range: Range<usize>) -> Result<&mut [u8], ValueError>;
    fn size(&self) -> usize;
}

pub struct OwnedMemory {
    data: Vec<u8>,
}

impl OwnedMemory {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
}

impl Memory for OwnedMemory {
    fn get_slice(&self, range: Range<usize>) -> Result<&[u8], ValueError> {
        if range.end <= self.data.len() && range.start <= range.end {
            Ok(&self.data[range])
        } else {
            Err(ValueError::AccessViolation)
        }
    }

    fn get_mut_slice(&mut self, range: Range<usize>) -> Result<&mut [u8], ValueError> {
        if range.end <= self.data.len() && range.start <= range.end {
            Ok(&mut self.data[range])
        } else {
            Err(ValueError::AccessViolation)
        }
    }

    fn size(&self) -> usize {
        self.data.len()
    }
}

pub struct Process<'a, M: Memory> {
    offset: usize,
    memory: &'a mut M,
}

impl<'a, M> Process<'a, M>
where
    M: Memory,
{
    pub fn new(memory: &'a mut M) -> Self {
        Self { memory, offset: 0 }
    }

    fn write_value(&mut self, value: Value) -> Result<(), ValueError> {
        let dst = self
            .memory
            .get_mut_slice(self.offset..self.offset + std::mem::size_of::<Value>())?;
        self.offset += std::mem::size_of::<Value>();
        dst.copy_from_slice(&value.bits().to_le_bytes());
        Ok(())
    }

    // pub fn string(&mut self, string: &str) -> Result<Value, ValueError> {
    //     let len = string.len();
    //     if len <= Self::INLINE_STRING_MAX {
    //         let dst = self.memory.get_mut_slice(self.offset..self.offset + len)?;
    //         dst.copy_from_slice(string.as_bytes());
    //         let addr = self.offset;
    //         self.offset += len;
    //         Ok(Value::new_ptr(ValueType::String, len as u16, addr))
    //     } else {
    //         Err(ValueError::StringTooLong)
    //     }
    // }

    pub fn string(&mut self, string: &str) -> Result<Value, ValueError> {
        let len = string.len();
        if len <= HASH_SIZE {
            let addr = self.offset;
            let dst = self.memory.get_mut_slice(addr..addr + HASH_SIZE)?;
            let src = string.as_bytes();
            for i in 0..HASH_SIZE {
                dst[i] = if i < len { src[i] } else { 0 };
            }
            self.offset += HASH_SIZE;
            Ok(Value::new_ptr(ValueType::String, len as u16, addr))
        } else {
            Err(ValueError::StringTooLong)
        }
    }

    pub fn block(&mut self, blk: &[Value]) -> Result<Value, ValueError> {
        let len = blk.len();
        let addr = self.offset;
        let value = Value::new_ptr(ValueType::Block, len as u16, addr);
        for &v in blk {
            self.write_value(v)?;
        }
        Ok(value)
    }

    // fn as_string_from_ptr(&self, value: Value) -> Result<&str, ValueError> {
    //     let addr = value.as_wasm_ptr();
    //     let len = value.payload_high_bits() as usize;
    //     if addr + len > self.memory.len() * PAGE_SIZE {
    //         Err(ValueError::AccessViolation)
    //     } else {
    //         Ok(unsafe {
    //             std::str::from_utf8_unchecked(&self.as_bytes().get_unchecked(addr..addr + len))
    //         })
    //     }
    // }

    // pub fn as_inline_string(&self, value: Value) -> Result<&str, ValueError> {
    //     match value.tag() {
    //         TAG_STRING => self.as_string_from_ptr(value),
    //         _ => Err(ValueError::TypeMismatch),
    //     }
    // }
}

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueType {
    Int = 0x0,
    Float = 0x1,
    Bytes = 0x2,
    String = 0x3,
    Block = 0x4,
    None = 0xf,
}

const TAG_STRING: u64 = ValueType::String as u64;

impl Value {
    pub fn none() -> Self {
        let fraction = FRACTION_TOP_BIT | Self::tag_bits(ValueType::None);
        let bits = (0 << 63) | EXP_MASK | fraction;
        Value(bits)
    }

    pub fn verify(bits: u64) -> Result<(), ValueError> {
        if (bits & QNAN_MASK) == QNAN_MASK {
            Ok(())
        } else {
            Err(ValueError::NotAValue)
        }
    }

    fn tag_bits(tag: ValueType) -> u64 {
        ((tag as u64) & TAG_MASK) << TAG_SHIFT
    }

    pub fn tag(&self) -> u64 {
        let bits = self.0;
        let fraction = bits & ((1 << 52) - 1); // lower 52 bits
        (fraction >> TAG_SHIFT) & TAG_MASK
    }

    pub fn new_int_unchecked(value: i64) -> Self {
        let payload_47 = ((value << (64 - 47)) >> (64 - 47)) as u64 & PAYLOAD_MASK_47;
        let fraction = FRACTION_TOP_BIT | Self::tag_bits(ValueType::Int) | payload_47;
        let bits = (0 << 63) | EXP_MASK | fraction;
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
            Err(ValueError::IntOutOfRange)
        }
    }

    /// Interpret this Value as a 47-bit signed integer.
    pub fn as_int(&self) -> i64 {
        let bits = self.0;
        let payload_47 = bits & PAYLOAD_MASK_47;
        let shifted = (payload_47 << (64 - 47)) as i64; // cast to i64 => preserve bits
        let value = shifted >> (64 - 47); // arithmetic shift right
        value
    }

    /// Create a boxed pointer (32 bits). Tag = Ptr, fraction bit 51=1, payload in bits 46..0.
    fn new_ptr(tag: ValueType, payload: u16, addr: usize) -> Self {
        let payload_47 = ((payload as u64) << 32) | ((addr as u64) & 0xFFFF_FFFF);
        let fraction = FRACTION_TOP_BIT
            | ((tag as u64) & TAG_MASK) << TAG_SHIFT
            | (payload_47 & PAYLOAD_MASK_47);
        let bits = (0 << 63) | EXP_MASK | fraction;
        Value(bits)
    }

    /// Return the pointer as 32 bits.
    pub fn as_wasm_ptr(&self) -> usize {
        (self.0 & 0xFFFF_FFFF) as usize
    }

    pub fn payload_high_bits(&self) -> u32 {
        ((self.0 & PAYLOAD_MASK_47) >> 32) as u32
    }

    /// Raw bits for debugging or advanced usage
    pub fn bits(&self) -> u64 {
        self.0
    }
}

#[inline(never)]
pub fn string_test_1(memory: &mut Process<OwnedMemory>, s: &str) -> Result<Value, ValueError> {
    memory.string(s)
}

#[inline(never)]
pub fn block_test_1(memory: &mut Process<OwnedMemory>, blk: &[Value]) -> Result<Value, ValueError> {
    memory.block(blk)
}

// #[inline(never)]
// pub fn string_test_2(memory: &mut OwnedMemory, v: Value) -> Result<&str, ValueError> {
//     memory.as_string(v)
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_round_trip() {
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
            let b = Value::new_int_unchecked(v);
            let back = b.as_int();
            assert_eq!(
                v,
                back,
                "Failed round-trip for {} => bits=0x{:016X} => {}",
                v,
                b.bits(),
                back
            );
        }
    }

    #[test]
    #[should_panic]
    #[allow(arithmetic_overflow)]
    fn test_int_out_of_range() {
        // +2^46 is out of range: 140,737,488,355,328
        Value::new_int_unchecked((1 << 46) as i64);
    }

    #[test]
    fn test_ptr_round_trip() {
        let ptrs = [0usize, 1, 0xDEAD_BEEF, 0xFFFF_FFFF];
        for &p in &ptrs {
            let b = Value::new_ptr(ValueType::String, 0, p);
            let back = b.as_wasm_ptr();
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
}
