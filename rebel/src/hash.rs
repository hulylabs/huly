#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Fast hash function optimized for arrays of exactly 8 u32 values
/// Uses SIMD instructions when available, falls back to scalar implementation otherwise
#[inline]
pub fn fast_hash(input: &[u32; 8]) -> u32 {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    unsafe {
        fast_hash_avx2(input)
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    unsafe {
        fast_hash_neon(input)
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    fast_hash_scalar(input)
}

/// AVX2 SIMD implementation of the hash function for x86_64
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn fast_hash_avx2(input: &[u32; 8]) -> u32 {
    // Constants for mixing
    const C1: u32 = 0xcc9e2d51;
    const C2: u32 = 0x1b873593;

    // Load input into AVX2 registers
    let data = _mm256_loadu_si256(input.as_ptr() as *const __m256i);

    // Multiply by constant C1
    let mut h1 = _mm256_mullo_epi32(data, _mm256_set1_epi32(C1 as i32));

    // Rotate left by 15
    h1 = _mm256_or_si256(_mm256_slli_epi32(h1, 15), _mm256_srli_epi32(h1, 17));

    // Multiply by constant C2
    h1 = _mm256_mullo_epi32(h1, _mm256_set1_epi32(C2 as i32));

    // Horizontal add to combine all lanes
    let sum = _mm256_hadd_epi32(h1, h1);
    let sum = _mm256_hadd_epi32(sum, sum);

    // Extract final hash
    let mut result = _mm256_extract_epi32(sum, 0) as u32;

    // Final mix
    result ^= result >> 16;
    result = result.wrapping_mul(0x85ebca6b);
    result ^= result >> 13;
    result = result.wrapping_mul(0xc2b2ae35);
    result ^= result >> 16;

    result
}

/// NEON SIMD implementation of the hash function for ARM64
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn fast_hash_neon(input: &[u32; 8]) -> u32 {
    // Constants for mixing
    const C1: u32 = 0xcc9e2d51;
    const C2: u32 = 0x1b873593;

    // Load input into NEON registers
    // Process in two chunks of 4 u32s since NEON works with 128-bit vectors
    let data1 = vld1q_u32(input[0..4].as_ptr());
    let data2 = vld1q_u32(input[4..8].as_ptr());

    // Multiply by C1
    let mut h1 = vmulq_n_u32(data1, C1);
    let mut h2 = vmulq_n_u32(data2, C1);

    // Rotate left by 15
    h1 = vorrq_u32(vshlq_n_u32(h1, 15), vshrq_n_u32(h1, 17));
    h2 = vorrq_u32(vshlq_n_u32(h2, 15), vshrq_n_u32(h2, 17));

    // Multiply by C2
    h1 = vmulq_n_u32(h1, C2);
    h2 = vmulq_n_u32(h2, C2);

    // Add the two halves
    let sum = vaddq_u32(h1, h2);

    // Horizontal add to combine all lanes
    let pair_sum = vpadd_u32(vget_low_u32(sum), vget_high_u32(sum));
    let final_sum = vpadd_u32(pair_sum, pair_sum);

    // Extract final hash
    let mut result = vget_lane_u32(final_sum, 0);

    // Final mix
    result ^= result >> 16;
    result = result.wrapping_mul(0x85ebca6b);
    result ^= result >> 13;
    result = result.wrapping_mul(0xc2b2ae35);
    result ^= result >> 16;

    result
}

/// Scalar fallback implementation
#[inline]
fn fast_hash_scalar(input: &[u32; 8]) -> u32 {
    const C1: u32 = 0xcc9e2d51;
    const C2: u32 = 0x1b873593;

    let mut h1: u32 = 0;

    for &k in input {
        let mut k1 = k.wrapping_mul(C1);
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(C2);

        h1 ^= k1;
        h1 = h1.rotate_left(13);
        h1 = h1.wrapping_mul(5).wrapping_add(0xe6546b64);
    }

    // Final mix
    h1 ^= h1 >> 16;
    h1 = h1.wrapping_mul(0x85ebca6b);
    h1 ^= h1 >> 13;
    h1 = h1.wrapping_mul(0xc2b2ae35);
    h1 ^= h1 >> 16;

    h1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_consistency() {
        let input = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let hash1 = fast_hash(&input);
        let hash2 = fast_hash(&input);
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

        let hashes: Vec<u32> = inputs.iter().map(|input| fast_hash(input)).collect();

        // Check that all hashes are different
        for i in 0..hashes.len() {
            for j in i + 1..hashes.len() {
                assert_ne!(hashes[i], hashes[j], "Hash collision detected");
            }
        }
    }
}
