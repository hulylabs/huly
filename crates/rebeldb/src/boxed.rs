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

/// Example tags
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Int = 0x0, // up to you which nibble you choose
    Ptr = 0x1,
    // up to 0xF ...
}

impl BoxedValue {
    /// Create a boxed *signed* integer with 47-bit 2's complement payload.
    ///
    /// Valid range: -2^46 .. 2^46 - 1
    /// (i.e. about ±140.7 trillion)
    pub fn new_int(value: i64) -> Self {
        // Check range
        let min = -(1 << 46); // -140,737,488,355,328
        let max = (1 << 46) - 1; // +140,737,488,355,327
        assert!(value >= min && value <= max, "Integer out of 47-bit range");

        // We want to store this i64 in the low 47 bits, 2's complement.
        // Easiest approach is to mask off lower 47 bits of the sign-extended i64.
        // 1) shift left 17, then arithmetic right 17 => sign-extend from bit 46
        // 2) cast to u64 => the bottom 47 bits contain the 2's complement form
        let payload_47 = ((value << (64 - 47)) >> (64 - 47)) as u64 & PAYLOAD_MASK_47;

        // Build fraction:
        //   bit 51 = 1
        //   bits 50..47 = Tag::Int
        //   bits 46..0 = payload_47
        let fraction = FRACTION_TOP_BIT | ((Tag::Int as u64) & TAG_MASK) << TAG_SHIFT | payload_47;

        // sign bit (63) = 0, exponent=0x7FF, fraction
        let bits = (0 << 63) | EXP_MASK | fraction;
        BoxedValue(bits)
    }

    /// Interpret this BoxedValue as a 47-bit signed integer.
    pub fn as_int(&self) -> i64 {
        let bits = self.0;

        // 1) Check exponent is 0x7FF
        let exponent = (bits >> EXP_SHIFT) & 0x7FF;
        assert_eq!(
            exponent, EXP_MAX,
            "Not a NaN exponent, can't be a NaN-boxed value."
        );

        // 2) Extract fraction
        let fraction = bits & ((1 << 52) - 1); // lower 52 bits

        // bit 51 must be 1
        assert!(
            (fraction >> 51) == 1,
            "Fraction bit 51 not set => Infinity or normal float."
        );

        // 3) Check tag
        let tag = (fraction >> TAG_SHIFT) & TAG_MASK;
        assert_eq!(tag, Tag::Int as u64, "Tag != Int");

        // 4) Extract the 47-bit payload, sign-extend from bit 46
        let payload_47 = fraction & PAYLOAD_MASK_47;

        // sign-extend from bit 46 => shift up, then arithmetic shift down
        let shifted = (payload_47 << (64 - 47)) as i64; // cast to i64 => preserve bits
        let value = shifted >> (64 - 47); // arithmetic shift right
        value
    }

    /// Create a boxed pointer (32 bits). Tag = Ptr, fraction bit 51=1, payload in bits 46..0.
    pub fn new_ptr(addr: u32) -> Self {
        let payload_47 = addr as u64; // zero-extended into 64
                                      // We could store a 46- or 47-bit pointer, but typically 32 bits is enough.

        let fraction = FRACTION_TOP_BIT
            | ((Tag::Ptr as u64) & TAG_MASK) << TAG_SHIFT
            | (payload_47 & PAYLOAD_MASK_47);

        let bits = (0 << 63) | EXP_MASK | fraction;
        BoxedValue(bits)
    }

    /// Return the pointer as 32 bits.
    pub fn as_ptr(&self) -> u32 {
        let bits = self.0;

        // exponent must be 0x7FF
        let exponent = (bits >> EXP_SHIFT) & 0x7FF;
        assert_eq!(
            exponent, EXP_MAX,
            "Not a NaN exponent => not a NaN-boxed value."
        );

        let fraction = bits & ((1 << 52) - 1);
        // bit 51 must be 1 => otherwise Infinity or normal float
        assert!(
            (fraction >> 51) == 1,
            "Fraction bit 51 not set => Infinity or normal float."
        );

        let tag = (fraction >> TAG_SHIFT) & TAG_MASK;
        assert_eq!(tag, Tag::Ptr as u64, "Tag != Ptr");

        // Just the lower 47 bits
        let payload_47 = fraction & PAYLOAD_MASK_47;
        payload_47 as u32
    }

    /// Raw bits for debugging or advanced usage
    pub fn bits(&self) -> u64 {
        self.0
    }
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
