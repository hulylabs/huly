// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

//! Variable-length integer encoding, optimized for small/frequent values
//!
//! This module provides utilities for encoding and decoding integers in a
//! compact variable-length format, similar to SQLite's encoding:
//!
//! - Single byte for common small values
//! - Progressively more bytes for larger values
//! - Sign-magnitude representation to handle negative numbers efficiently

/// Encodes an i32 value into a buffer using a variable-length encoding scheme.
///
/// The encoding scheme works as follows:
/// - Values 0-63: Encoded in a single byte (0xxxxxxx)
/// - Values -1 to -64: Encoded in a single byte (1xxxxxxx, where xxxxxxx is the negative offset from -1)
/// - Larger values: 1-4 bytes with a tag indicating length and sign
///
/// # Arguments
/// * `value` - The i32 value to encode
/// * `buffer` - The destination buffer to write into (must be at least 5 bytes long)
///
/// # Returns
/// The number of bytes written to the buffer
///
/// # Panics
/// Will panic if the buffer is too small (less than 5 bytes)
pub fn encode_i32(value: i32, buffer: &mut [u8]) -> usize {
    assert!(buffer.len() >= 5, "Buffer must be at least 5 bytes");

    match value {
        // Small positive values (0-63): Single byte with high bit unset
        0..=63 => {
            buffer[0] = value as u8;
            1
        }

        // Small negative values (-1 to -64): Single byte with high bit set
        -64..=-1 => {
            // Encode -1 as 0x80, -2 as 0x81, etc.
            buffer[0] = 0x80 | ((-value - 1) as u8);
            1
        }

        // Larger positive values: Multiple bytes with tag
        64..=127 => {
            buffer[0] = 0x40; // Tag: 01000000 (one byte positive)
            buffer[1] = value as u8;
            2
        }
        128..=32767 => {
            buffer[0] = 0x41; // Tag: 01000001 (two bytes positive)
            buffer[1] = (value >> 8) as u8;
            buffer[2] = value as u8;
            3
        }
        32768..=8388607 => {
            buffer[0] = 0x42; // Tag: 01000010 (three bytes positive)
            buffer[1] = (value >> 16) as u8;
            buffer[2] = (value >> 8) as u8;
            buffer[3] = value as u8;
            4
        }
        // Larger values requiring full 4 bytes
        _ if value > 0 => {
            buffer[0] = 0x43; // Tag: 01000011 (four bytes positive)
            buffer[1] = (value >> 24) as u8;
            buffer[2] = (value >> 16) as u8;
            buffer[3] = (value >> 8) as u8;
            buffer[4] = value as u8;
            5
        }
        // Larger negative values: Multiple bytes with tag
        -128..=-65 => {
            buffer[0] = 0x44; // Tag: 01000100 (one byte negative)
            buffer[1] = (-value) as u8;
            2
        }
        -32768..=-129 => {
            buffer[0] = 0x45; // Tag: 01000101 (two bytes negative)
            let abs_val = -value;
            buffer[1] = (abs_val >> 8) as u8;
            buffer[2] = abs_val as u8;
            3
        }
        -8388608..=-32769 => {
            buffer[0] = 0x46; // Tag: 01000110 (three bytes negative)
            let abs_val = -value;
            buffer[1] = (abs_val >> 16) as u8;
            buffer[2] = (abs_val >> 8) as u8;
            buffer[3] = abs_val as u8;
            4
        }
        // Negative values requiring full 4 bytes
        _ => {
            buffer[0] = 0x47; // Tag: 01000111 (four bytes negative)
            // Be careful with i32::MIN which can't be negated directly
            let abs_val = if value == i32::MIN {
                value as u32
            } else {
                (-value) as u32
            };
            buffer[1] = (abs_val >> 24) as u8;
            buffer[2] = (abs_val >> 16) as u8;
            buffer[3] = (abs_val >> 8) as u8;
            buffer[4] = abs_val as u8;
            5
        }
    }
}

/// Decodes a variable-length encoded integer from a buffer.
///
/// # Arguments
/// * `buffer` - The buffer containing the encoded value
///
/// # Returns
/// A tuple of (decoded value, number of bytes read)
///
/// # Panics
/// Will panic if the buffer is too small for the encoded value
pub fn decode_i32(buffer: &[u8]) -> (i32, usize) {
    assert!(!buffer.is_empty(), "Buffer must not be empty");

    let first_byte = buffer[0];

    match first_byte {
        // Small positive values (0-63): Single byte with high bit unset
        0..=0x3F => (first_byte as i32, 1),

        // Small negative values (-1 to -64): Single byte with high bit set
        0x80..=0xBF => {
            // Decode 0x80 as -1, 0x81 as -2, etc.
            let negative_offset = (first_byte & 0x7F) as i32;
            (-negative_offset - 1, 1)
        }

        // Larger values with tags
        0x40 => {
            assert!(buffer.len() >= 2, "Buffer too small for encoded value");
            (buffer[1] as i32, 2)
        }
        0x41 => {
            assert!(buffer.len() >= 3, "Buffer too small for encoded value");
            let value = ((buffer[1] as i32) << 8) | (buffer[2] as i32);
            (value, 3)
        }
        0x42 => {
            assert!(buffer.len() >= 4, "Buffer too small for encoded value");
            let value = ((buffer[1] as i32) << 16) | ((buffer[2] as i32) << 8) | (buffer[3] as i32);
            (value, 4)
        }
        0x43 => {
            assert!(buffer.len() >= 5, "Buffer too small for encoded value");
            let value = ((buffer[1] as i32) << 24)
                | ((buffer[2] as i32) << 16)
                | ((buffer[3] as i32) << 8)
                | (buffer[4] as i32);
            (value, 5)
        }

        // Larger negative values with tags
        0x44 => {
            assert!(buffer.len() >= 2, "Buffer too small for encoded value");
            (-(buffer[1] as i32), 2)
        }
        0x45 => {
            assert!(buffer.len() >= 3, "Buffer too small for encoded value");
            let value = ((buffer[1] as i32) << 8) | (buffer[2] as i32);
            (-value, 3)
        }
        0x46 => {
            assert!(buffer.len() >= 4, "Buffer too small for encoded value");
            let value = ((buffer[1] as i32) << 16) | ((buffer[2] as i32) << 8) | (buffer[3] as i32);
            (-value, 4)
        }
        0x47 => {
            assert!(buffer.len() >= 5, "Buffer too small for encoded value");
            let value = ((buffer[1] as i32) << 24)
                | ((buffer[2] as i32) << 16)
                | ((buffer[3] as i32) << 8)
                | (buffer[4] as i32);
            (-value, 5)
        }

        // Invalid tag
        _ => panic!("Invalid tag in encoded value: {}", first_byte),
    }
}

/// Calculates the number of bytes needed to encode a given i32 value
pub fn encoded_size(value: i32) -> usize {
    match value {
        0..=63 => 1,      // Small positive values
        -64..=-1 => 1,    // Small negative values
        64..=127 => 2,    // Positive values fitting in 1 byte after tag
        -128..=-65 => 2,  // Negative values fitting in 1 byte after tag
        128..=32767 => 3, // Positive values fitting in 2 bytes after tag
        -32768..=-129 => 3, // Negative values fitting in 2 bytes after tag
        32768..=8388607 => 4, // Positive values fitting in 3 bytes after tag
        -8388608..=-32769 => 4, // Negative values fitting in 3 bytes after tag
        _ => 5,           // Values requiring 4 bytes after tag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_small_positive() {
        let mut buffer = [0u8; 5];
        
        // Test encoding 0
        let bytes_written = encode_i32(0, &mut buffer);
        assert_eq!(bytes_written, 1);
        assert_eq!(buffer[0], 0);
        
        // Test encoding 42
        let bytes_written = encode_i32(42, &mut buffer);
        assert_eq!(bytes_written, 1);
        assert_eq!(buffer[0], 42);
        
        // Test encoding 63 (max single-byte positive)
        let bytes_written = encode_i32(63, &mut buffer);
        assert_eq!(bytes_written, 1);
        assert_eq!(buffer[0], 63);
    }
    
    #[test]
    fn test_encode_small_negative() {
        let mut buffer = [0u8; 5];
        
        // Test encoding -1
        let bytes_written = encode_i32(-1, &mut buffer);
        assert_eq!(bytes_written, 1);
        assert_eq!(buffer[0], 0x80);
        
        // Test encoding -42
        let bytes_written = encode_i32(-42, &mut buffer);
        assert_eq!(bytes_written, 1);
        assert_eq!(buffer[0], 0x80 + 41);
        
        // Test encoding -64 (max single-byte negative)
        let bytes_written = encode_i32(-64, &mut buffer);
        assert_eq!(bytes_written, 1);
        assert_eq!(buffer[0], 0x80 + 63);
    }
    
    #[test]
    fn test_encode_larger_values() {
        let mut buffer = [0u8; 5];
        
        // Test encoding 100 (positive)
        let bytes_written = encode_i32(100, &mut buffer);
        assert_eq!(bytes_written, 2);
        assert_eq!(buffer[0], 0x40);
        assert_eq!(buffer[1], 100);
        
        // Test encoding 1000 (positive)
        let bytes_written = encode_i32(1000, &mut buffer);
        assert_eq!(bytes_written, 3);
        assert_eq!(buffer[0], 0x41);
        assert_eq!(buffer[1], 0x03);
        assert_eq!(buffer[2], 0xE8);
        
        // Test encoding 100000 (positive)
        let bytes_written = encode_i32(100000, &mut buffer);
        assert_eq!(bytes_written, 4);
        assert_eq!(buffer[0], 0x42);
        assert_eq!(buffer[1], 0x01);
        assert_eq!(buffer[2], 0x86);
        assert_eq!(buffer[3], 0xA0);
        
        // Test encoding 2000000000 (positive)
        let bytes_written = encode_i32(2000000000, &mut buffer);
        assert_eq!(bytes_written, 5);
        assert_eq!(buffer[0], 0x43);
        assert_eq!(buffer[1], 0x77);
        assert_eq!(buffer[2], 0x35);
        assert_eq!(buffer[3], 0x94);
        assert_eq!(buffer[4], 0x00);
    }
    
    #[test]
    fn test_encode_larger_negative_values() {
        let mut buffer = [0u8; 5];
        
        // Test encoding -100 (negative)
        let bytes_written = encode_i32(-100, &mut buffer);
        assert_eq!(bytes_written, 2);
        assert_eq!(buffer[0], 0x44);
        assert_eq!(buffer[1], 100);
        
        // Test encoding -1000 (negative)
        let bytes_written = encode_i32(-1000, &mut buffer);
        assert_eq!(bytes_written, 3);
        assert_eq!(buffer[0], 0x45);
        assert_eq!(buffer[1], 0x03);
        assert_eq!(buffer[2], 0xE8);
        
        // Test encoding -100000 (negative)
        let bytes_written = encode_i32(-100000, &mut buffer);
        assert_eq!(bytes_written, 4);
        assert_eq!(buffer[0], 0x46);
        assert_eq!(buffer[1], 0x01);
        assert_eq!(buffer[2], 0x86);
        assert_eq!(buffer[3], 0xA0);
        
        // Test encoding -2000000000 (negative)
        let bytes_written = encode_i32(-2000000000, &mut buffer);
        assert_eq!(bytes_written, 5);
        assert_eq!(buffer[0], 0x47);
        assert_eq!(buffer[1], 0x77);
        assert_eq!(buffer[2], 0x35);
        assert_eq!(buffer[3], 0x94);
        assert_eq!(buffer[4], 0x00);
        
        // Test encoding i32::MIN (negative edge case)
        let bytes_written = encode_i32(i32::MIN, &mut buffer);
        assert_eq!(bytes_written, 5);
        assert_eq!(buffer[0], 0x47);
        // i32::MIN is -2147483648, so the bytes should be 0x80000000
        assert_eq!(buffer[1], 0x80);
        assert_eq!(buffer[2], 0x00);
        assert_eq!(buffer[3], 0x00);
        assert_eq!(buffer[4], 0x00);
    }
    
    #[test]
    fn test_decode() {
        let mut buffer = [0u8; 5];
        
        // Test round-trip for various values
        let test_values = [
            0, 1, 42, 63, 64, 100, 127, 128, 1000, 32767, 32768, 100000, 2000000000,
            -1, -42, -64, -65, -100, -1000, -32768, -32769, -100000, -2000000000,
        ];
        
        for &value in &test_values {
            let bytes_written = encode_i32(value, &mut buffer);
            let (decoded, bytes_read) = decode_i32(&buffer);
            
            assert_eq!(decoded, value, "Failed to round-trip value {}", value);
            assert_eq!(bytes_read, bytes_written, "Bytes read != bytes written for value {}", value);
            assert_eq!(encoded_size(value), bytes_written, "encoded_size doesn't match actual bytes written for {}", value);
        }
    }
    
    #[test]
    fn test_encoded_size() {
        // Test small values (1 byte)
        assert_eq!(encoded_size(0), 1);
        assert_eq!(encoded_size(63), 1);
        assert_eq!(encoded_size(-1), 1);
        assert_eq!(encoded_size(-64), 1);
        
        // Test medium values (2-3 bytes)
        assert_eq!(encoded_size(64), 2);
        assert_eq!(encoded_size(127), 2);
        assert_eq!(encoded_size(-65), 2);
        assert_eq!(encoded_size(128), 3);
        assert_eq!(encoded_size(32767), 3);
        assert_eq!(encoded_size(-128), 2);
        assert_eq!(encoded_size(-32768), 3);
        
        // Test large values (4-5 bytes)
        assert_eq!(encoded_size(32768), 4);
        assert_eq!(encoded_size(8388607), 4);
        assert_eq!(encoded_size(-32769), 4);
        assert_eq!(encoded_size(8388608), 5);
        assert_eq!(encoded_size(-8388608), 4);
        assert_eq!(encoded_size(-8388609), 5);
        assert_eq!(encoded_size(2147483647), 5);  // i32::MAX
        assert_eq!(encoded_size(-2147483648), 5); // i32::MIN
    }
}