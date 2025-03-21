// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

#[cfg(target_feature = "sse4.2")]
use std::arch::x86_64::*;

#[cfg(target_feature = "crc")]
use std::arch::aarch64::*;

#[cfg(all(target_arch = "x86_64", target_feature = "sse4.2"))]
pub fn hash_u32x8(input: &[u32; 8]) -> u32 {
    unsafe {
        let mut hash: u32 = 0;
        for &value in input {
            hash = _mm_crc32_u32(hash, value);
        }
        hash
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "crc"))]
pub fn hash_u32x8(input: &[u32; 8]) -> u32 {
    unsafe {
        let mut hash: u32 = 0;
        for &value in input {
            hash = __crc32w(hash, value);
        }
        hash
    }
}

#[cfg(not(any(
    all(target_arch = "x86_64", target_feature = "sse4.2"),
    all(target_arch = "aarch64", target_feature = "crc")
)))]
pub fn hash_u32x8(input: &[u32; 8]) -> u32 {
    let mut h: u32 = 0x811C9DC5;
    const PRIME: u32 = 0x01000193;

    for &value in input {
        h ^= value;
        h = h.wrapping_mul(PRIME);
    }

    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_consistency() {
        let input = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let hash1 = hash_u32x8(&input);
        let hash2 = hash_u32x8(&input);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_distribution() {
        let inputs = [
            [1u32; 8],
            [2u32; 8],
            [0u32, 1, 2, 3, 4, 5, 6, 7],
            [7u32, 6, 5, 4, 3, 2, 1, 0],
        ];

        let hashes: Vec<u32> = inputs.iter().map(|input| hash_u32x8(input)).collect();

        // Check that all hashes are different
        for i in 0..hashes.len() {
            for j in i + 1..hashes.len() {
                assert_ne!(hashes[i], hashes[j], "Hash collision detected");
            }
        }
    }

    #[test]
    fn test_avalanche() {
        let base = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let base_hash = hash_u32x8(&base);

        // Test that changing any single bit causes significant changes
        for pos in 0..8 {
            let mut modified = base;
            modified[pos] ^= 1;
            let modified_hash = hash_u32x8(&modified);

            // Ensure the hashes are different
            assert_ne!(base_hash, modified_hash);

            // Count differing bits (should be close to 16 for good avalanche)
            let diff_bits = (base_hash ^ modified_hash).count_ones();
            assert!(diff_bits >= 10, "Poor avalanche effect detected");
        }
    }
}
