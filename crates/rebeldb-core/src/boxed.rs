//

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BoxedError {
    #[error("Integer out of 47-bit range")]
    IntOutOfRange,
    #[error("Not a NaN-boxed value")]
    NotAQNan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxedValue(u64);

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

/// Example tags
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Int = 0x0,
    WasmPtr = 0x1,
    Float = 0x2,
    Object = 0x3,
}

impl BoxedValue {
    /// Create a boxed *signed* integer with 47-bit 2's complement payload.
    ///
    /// Valid range: -2^46 .. 2^46 - 1
    /// (i.e. about ±140.7 trillion)
    pub fn new_int(value: i64) -> Self {
        let payload_47 = ((value << (64 - 47)) >> (64 - 47)) as u64 & PAYLOAD_MASK_47;
        let fraction = FRACTION_TOP_BIT | ((Tag::Int as u64) & TAG_MASK) << TAG_SHIFT | payload_47;
        let bits = (0 << 63) | EXP_MASK | fraction;
        BoxedValue(bits)
    }

    /// Create a boxed *signed* integer with 47-bit 2's complement payload.
    ///
    /// Valid range: -2^46 .. 2^46 - 1
    /// (i.e. about ±140.7 trillion)
    pub fn safe_new_int(value: i64) -> Result<Self, BoxedError> {
        let min = -(1 << 46); // -140,737,488,355,328
        let max = (1 << 46) - 1; // +140,737,488,355,327
        if value >= min && value <= max {
            Ok(Self::new_int(value))
        } else {
            Err(BoxedError::IntOutOfRange)
        }
    }

    /// Interpret this BoxedValue as a 47-bit signed integer.
    pub fn as_int(&self) -> i64 {
        let bits = self.0;
        let payload_47 = bits & PAYLOAD_MASK_47;
        let shifted = (payload_47 << (64 - 47)) as i64; // cast to i64 => preserve bits
        let value = shifted >> (64 - 47); // arithmetic shift right
        value
    }

    /// Interpret this BoxedValue as a 47-bit signed integer.
    pub fn verify_nan(bits: u64) -> Result<(), BoxedError> {
        if (bits & QNAN_MASK) == QNAN_MASK {
            Ok(())
        } else {
            Err(BoxedError::NotAQNan)
        }
    }

    pub fn tag(&self) -> u8 {
        let bits = self.0;
        let fraction = bits & ((1 << 52) - 1); // lower 52 bits
        ((fraction >> TAG_SHIFT) & TAG_MASK) as u8
    }

    /// Create a boxed pointer (32 bits). Tag = Ptr, fraction bit 51=1, payload in bits 46..0.
    pub fn new_ptr(addr: u32) -> Self {
        let payload_47 = addr as u64; // zero-extended into 64
                                      // We could store a 46- or 47-bit pointer, but typically 32 bits is enough.

        let fraction = FRACTION_TOP_BIT
            | ((Tag::WasmPtr as u64) & TAG_MASK) << TAG_SHIFT
            | (payload_47 & PAYLOAD_MASK_47);

        let bits = (0 << 63) | EXP_MASK | fraction;
        BoxedValue(bits)
    }

    /// Return the pointer as 32 bits.
    pub fn as_ptr(&self) -> u32 {
        let bits = self.0;
        let payload_47 = bits & PAYLOAD_MASK_47;
        payload_47 as u32
    }

    /// Raw bits for debugging or advanced usage
    pub fn bits(&self) -> u64 {
        self.0
    }
}

#[inline(never)]
pub fn tag(b: BoxedValue) -> u8 {
    b.tag()
}

#[inline(never)]
pub fn verify(value: u64) -> Result<(), BoxedError> {
    BoxedValue::verify_nan(value)
}

#[inline(never)]
pub fn box_int(value: i64) -> BoxedValue {
    BoxedValue::new_int(value)
}

#[inline(never)]
pub fn safe_box_int(value: i64) -> Result<BoxedValue, BoxedError> {
    BoxedValue::safe_new_int(value)
}

#[inline(never)]
pub fn unbox_int(b: BoxedValue) -> i64 {
    b.as_int()
}

#[inline(never)]
pub fn box_ptr(addr: u32) -> BoxedValue {
    BoxedValue::new_ptr(addr)
}

#[inline(never)]
pub fn unbox_ptr(b: BoxedValue) -> u32 {
    b.as_ptr()
}

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
            let b = BoxedValue::new_int(v);
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
        BoxedValue::new_int((1 << 46) as i64);
    }

    #[test]
    fn test_ptr_round_trip() {
        let ptrs = [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF];
        for &p in &ptrs {
            let b = BoxedValue::new_ptr(p);
            let back = b.as_ptr();
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
