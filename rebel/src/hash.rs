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
    // Better mixing constants from xxHash
    const P1: u32 = 0x9e3779b1;
    const P2: u32 = 0x85ebca77;
    const P3: u32 = 0xc2b2ae3d;

    // Load input into AVX2 registers
    let data = _mm256_loadu_si256(input.as_ptr() as *const __m256i);

    // First round of mixing
    let mut h1 = _mm256_mullo_epi32(data, _mm256_set1_epi32(P1 as i32));

    // Rotate and mix
    h1 = _mm256_or_si256(_mm256_slli_epi32(h1, 13), _mm256_srli_epi32(h1, 19));

    h1 = _mm256_mullo_epi32(h1, _mm256_set1_epi32(P2 as i32));

    // Second round of mixing
    h1 = _mm256_xor_si256(h1, _mm256_srli_epi32(h1, 16));

    // Horizontal add with extra mixing
    let sum1 = _mm256_hadd_epi32(h1, h1);
    let sum2 = _mm256_hadd_epi32(sum1, sum1);

    let mut result = _mm256_extract_epi32(sum2, 0) as u32;

    // Final avalanche
    result = result.wrapping_add(result.rotate_left(13));
    result = result.wrapping_mul(P1);
    result ^= result >> 17;
    result = result.wrapping_mul(P2);
    result ^= result >> 13;
    result = result.wrapping_mul(P3);
    result ^= result >> 16;

    result
}

/// NEON SIMD implementation of the hash function for ARM64
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn fast_hash_neon(input: &[u32; 8]) -> u32 {
    // Better mixing constants from xxHash
    const P1: u32 = 0x9e3779b1;
    const P2: u32 = 0x85ebca77;
    const P3: u32 = 0xc2b2ae3d;

    // Load input into NEON registers
    let data1 = vld1q_u32(input[0..4].as_ptr());
    let data2 = vld1q_u32(input[4..8].as_ptr());

    // First round of mixing
    let mut h1 = vmulq_n_u32(data1, P1);
    let mut h2 = vmulq_n_u32(data2, P1);

    // Rotate and mix
    h1 = vorrq_u32(vshlq_n_u32(h1, 13), vshrq_n_u32(h1, 19));
    h2 = vorrq_u32(vshlq_n_u32(h2, 13), vshrq_n_u32(h2, 19));

    h1 = vmulq_n_u32(h1, P2);
    h2 = vmulq_n_u32(h2, P2);

    // Second round of mixing
    h1 = veorq_u32(h1, vshrq_n_u32(h1, 16));
    h2 = veorq_u32(h2, vshrq_n_u32(h2, 16));

    // Combine vectors with additional mixing
    let combined = vaddq_u32(h1, vorrq_u32(vshlq_n_u32(h2, 13), vshrq_n_u32(h2, 19)));

    // Horizontal add with mixing
    let pair_sum = vpadd_u32(vget_low_u32(combined), vget_high_u32(combined));
    let final_sum = vpadd_u32(pair_sum, pair_sum);

    let mut result = vget_lane_u32(final_sum, 0);

    // Final avalanche
    result = result.wrapping_add(result.rotate_left(13));
    result = result.wrapping_mul(P1);
    result ^= result >> 17;
    result = result.wrapping_mul(P2);
    result ^= result >> 13;
    result = result.wrapping_mul(P3);
    result ^= result >> 16;

    result
}

/// Scalar fallback implementation
#[cfg(not(any(
    all(target_arch = "x86_64", target_feature = "avx2"),
    all(target_arch = "aarch64", target_feature = "neon")
)))]
#[inline]
fn fast_hash_scalar(input: &[u32; 8]) -> u32 {
    // Better mixing constants from xxHash
    const P1: u32 = 0x9e3779b1;
    const P2: u32 = 0x85ebca77;
    const P3: u32 = 0xc2b2ae3d;

    let mut h1: u32 = P1;

    for (i, &k) in input.iter().enumerate() {
        let k1 = k.wrapping_mul(P1);
        h1 ^= k1.rotate_left(13);
        h1 = h1.rotate_left(13).wrapping_add(h1.wrapping_mul(5));
        h1 = h1.wrapping_add(i as u32 + 1); // Position-sensitive mixing
    }

    // Final avalanche
    h1 = h1.wrapping_add(h1.rotate_left(13));
    h1 = h1.wrapping_mul(P1);
    h1 ^= h1 >> 17;
    h1 = h1.wrapping_mul(P2);
    h1 ^= h1 >> 13;
    h1 = h1.wrapping_mul(P3);
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

    #[test]
    fn test_avalanche() {
        let base = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let base_hash = fast_hash(&base);

        // Test that changing any single bit causes significant changes
        for pos in 0..8 {
            let mut modified = base;
            modified[pos] ^= 1;
            let modified_hash = fast_hash(&modified);

            // Ensure the hashes are different
            assert_ne!(base_hash, modified_hash);

            // Count differing bits (should be close to 16 for good avalanche)
            let diff_bits = (base_hash ^ modified_hash).count_ones();
            assert!(diff_bits >= 10, "Poor avalanche effect detected");
        }
    }
}
