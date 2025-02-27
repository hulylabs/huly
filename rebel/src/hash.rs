// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{_mm_crc32_u32, _mm_crc32_u64};

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::__crc32cw;

/// Compile-time CRC32C table generation using a `const fn`
/// Used internally to generate CRC32C lookup table at compile time
#[allow(dead_code)]
const fn generate_crc32c_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;

    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            crc = if (crc & 1) != 0 {
                0x82F63B78 ^ (crc >> 1) // CRC32C polynomial
            } else {
                crc >> 1
            };
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// Precomputed CRC32C lookup table (computed at compile time)
/// Used by the constant-time CRC32C hash implementation
#[allow(dead_code)]
const CRC32C_TABLE: [u32; 256] = generate_crc32c_table();

/// Computes a 32-bit hash for an 8-element u32 array
pub fn hash_u32x8(input: [u32; 8]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        hash_u32x8_x86(input)
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        hash_u32x8_arm(input)
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    hash_u32x8_const(input)
}

// -------------------- X86-64 (SSE4.2) IMPLEMENTATION --------------------
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
unsafe fn hash_u32x8_x86(input: [u32; 8]) -> u32 {
    let mut crc: u32 = 0;
    let ptr = input.as_ptr() as *const u64;

    crc = _mm_crc32_u64(crc as u64, *ptr) as u32;
    crc = _mm_crc32_u64(crc as u64, *ptr.add(1)) as u32;
    crc = _mm_crc32_u64(crc as u64, *ptr.add(2)) as u32;
    crc = _mm_crc32_u64(crc as u64, *ptr.add(3)) as u32;

    crc
}

// -------------------- AARCH64 (CRC32C) IMPLEMENTATION --------------------
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "crc")]
unsafe fn hash_u32x8_arm(input: [u32; 8]) -> u32 {
    let mut crc: u32 = 0;

    for &val in &input {
        crc = __crc32cw(crc, val);
    }

    crc
}

// -------------------- SCALAR FALLBACK (CRC32C TABLE) --------------------
// Software CRC32C implementation using a lookup table (identical to x86 & ARM CRC32C)
// fn hash_u32x8_scalar(input: [u32; 8]) -> u32 {
//     let mut crc: u32 = 0;
//     for &val in &input {
//         let mut v = val;
//         for _ in 0..4 {
//             let byte = (v & 0xFF) as u8;
//             v >>= 8;
//             let tbl_idx = (crc ^ (byte as u32)) & 0xFF;
//             crc = CRC32C_TABLE[tbl_idx as usize] ^ (crc >> 8);
//         }
//     }
//     crc
// }

/// Computes a CRC32C hash for a 8-element `u32` array at **compile time**.
/// This function is used as a fallback for architectures without hardware CRC32C support
/// and for computing hash values during compile time in tests
#[allow(dead_code)]
pub const fn hash_u32x8_const(input: [u32; 8]) -> u32 {
    let mut crc: u32 = 0;
    let mut i = 0;

    while i < 8 {
        let mut v = input[i];
        let mut j = 0;
        while j < 4 {
            let byte = (v & 0xFF) as u8;
            v >>= 8;
            let tbl_idx = (crc ^ (byte as u32)) & 0xFF;
            crc = CRC32C_TABLE[tbl_idx as usize] ^ (crc >> 8);
            j += 1;
        }
        i += 1;
    }

    crc
}

//

// pub fn hash_1(input: [u32; 8]) -> u32 {
//     hash_u32x8_scalar(input)
// }

// pub fn hash_2(input: [u32; 8]) -> u32 {
//     hash_u32x8_const(input)
// }

// -------------------- TESTS --------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_fn_hash() {
        const INPUT: [u32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        const HASH: u32 = hash_u32x8_const(INPUT);

        let runtime_hash = hash_u32x8(INPUT);
        assert_eq!(
            HASH, runtime_hash,
            "Compile-time and runtime hash must match"
        );
    }

    #[test]
    fn test_hash_consistency() {
        let inputs: [[u32; 8]; 5] = [
            [0, 0, 0, 0, 0, 0, 0, 0],                                     // All zeros
            [1, 2, 3, 4, 5, 6, 7, 8],                                     // Sequential values
            [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0, 0, 0, 0], // High vs. Low
            [
                0x12345678, 0x9ABCDEF0, 0x0FEDCBA9, 0x87654321, 0xCAFEBABE, 0xDEADBEEF, 0xFEEDFACE,
                0xBADC0DE,
            ], // Random mix
            [
                0xA5A5A5A5, 0x5A5A5A5A, 0xA5A5A5A5, 0x5A5A5A5A, 0xA5A5A5A5, 0x5A5A5A5A, 0xA5A5A5A5,
                0x5A5A5A5A,
            ], // Alternating pattern
        ];

        for &input in &inputs {
            let scalar_result = hash_u32x8_const(input);
            let optimized_result = hash_u32x8(input);
            assert_eq!(
                scalar_result, optimized_result,
                "Mismatch in hash for input: {:?}\nExpected: {:08x}, Got: {:08x}",
                input, scalar_result, optimized_result
            );
        }
    }

    #[test]
    fn test_hash_different_inputs() {
        let input1 = [1, 2, 3, 4, 5, 6, 7, 8];
        let input2 = [8, 7, 6, 5, 4, 3, 2, 1];

        let hash1 = hash_u32x8(input1);
        let hash2 = hash_u32x8(input2);

        assert_ne!(
            hash1, hash2,
            "Different inputs should produce different hashes"
        );
    }

    #[test]
    fn test_hash_stability() {
        let input = [42, 99, 123, 456, 789, 1024, 2048, 4096];
        let expected = hash_u32x8_const(input);
        let actual = hash_u32x8(input);

        assert_eq!(expected, actual, "Hashes should be stable across runs");
    }

    #[test]
    fn test_avalanche() {
        let base = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let base_hash = hash_u32x8(base);
        for i in 0..8 {
            let mut modified = base;
            modified[i] ^= 1;
            let modified_hash = hash_u32x8(modified);
            assert_ne!(
                base_hash, modified_hash,
                "Bit flip did not change the hash at index {}",
                i
            );
            let diff = (base_hash ^ modified_hash).count_ones();
            assert!(
                diff >= 10,
                "Avalanche effect too weak at index {}: diff = {}",
                i,
                diff
            );
        }
    }

    #[test]
    fn test_hash_edge_cases() {
        let zeros = [0u32; 8];
        let ones = [u32::MAX; 8];
        assert_ne!(
            hash_u32x8(zeros),
            hash_u32x8(ones),
            "Zeros and ones should yield different hash"
        );
    }

    #[test]
    fn test_hash_alternating() {
        let alt1 = [0, u32::MAX, 0, u32::MAX, 0, u32::MAX, 0, u32::MAX];
        let alt2 = [u32::MAX, 0, u32::MAX, 0, u32::MAX, 0, u32::MAX, 0];
        assert_ne!(
            hash_u32x8(alt1),
            hash_u32x8(alt2),
            "Alternating inputs yield same hash"
        );
    }
}
