// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

pub fn hash(arr: &[u32; 8]) -> u32 {
    let mut hash = 0x811c9dc5u32;

    const PRIMES: [u32; 8] = [
        0x85ebca77, 0xc2b2ae35, 0x27d4eb2f, 0x165667b1, 0xd3a99177, 0xa9bcae53, 0x71d13517,
        0xfd7046c5,
    ];

    for i in 0..8 {
        hash ^= arr[i].rotate_right(i as u32 * 3); // Rotate input before XOR
        hash = hash.rotate_left(13);
        hash = hash.wrapping_mul(PRIMES[i]);
        hash ^= hash.rotate_right(17); // Additional mixing step
    }

    // Final mixing
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85ebca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2ae35);
    hash ^= hash >> 16;

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_consistency() {
        let input = [1u32, 2, 3, 4, 5, 6, 7, 8];
        let hash1 = hash(&input);
        let hash2 = hash(&input);
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

        let hashes: Vec<u32> = inputs.iter().map(|input| hash(input)).collect();

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
        let base_hash = hash(&base);

        // Test that changing any single bit causes significant changes
        for pos in 0..8 {
            let mut modified = base;
            modified[pos] ^= 1;
            let modified_hash = hash(&modified);

            // Ensure the hashes are different
            assert_ne!(base_hash, modified_hash);

            // Count differing bits (should be close to 16 for good avalanche)
            let diff_bits = (base_hash ^ modified_hash).count_ones();
            assert!(diff_bits >= 10, "Poor avalanche effect detected");
        }
    }
}
