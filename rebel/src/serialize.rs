// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

//! Serialization for Value types
//!
//! This module provides a Visitor pattern-based approach to serializing Values
//! into different formats. This is the counterpart to the Collector pattern used
//! for parsing strings into Values.
//!
//! The serialization system consists of two main components:
//!
//! 1. **Serializers**: Implement the `Serializer` trait to define how values are
//!    written to a specific format. Each serializer defines its own error type
//!    and handles the specifics of the format.
//!
//! 2. **Extension trait**: The `ValueSerialize` trait is implemented for the `Value`
//!    type to enable serialization to any format by walking the structure and calling
//!    the appropriate serializer methods.
//!
//! # Example: Binary Serialization
//!
//! ```
//! use rebel::collector::parse;
//! use rebel::serialize::{to_bytes, from_bytes};
//! 
//! // Create a Value
//! let value = parse("[a: 42 b: \"hello\" [1 2 3]]").unwrap();
//! 
//! // Serialize to bytes
//! let bytes = to_bytes(&value).unwrap();
//! 
//! // Deserialize from bytes
//! let deserialized = from_bytes(&bytes).unwrap();
//! 
//! // Verify round-trip
//! assert_eq!(value, deserialized);
//! ```
//!
//! The binary format is compact and efficient:
//! - Type tags match the Tag constants in core.rs
//! - Integers use variable-length encoding (1-5 bytes)
//! - Strings include a length prefix followed by UTF-8 bytes
//! - Blocks include a length prefix followed by serialized items
//!
//! # Creating a Custom Serializer
//!
//! To create a new serializer format:
//!
//! 1. Define a struct that implements the `Serializer` trait
//! 2. Implement the required methods to handle each value type
//! 3. Use the `ValueSerialize` trait to serialize values

use crate::core::Tag;
use crate::encoding;
use crate::value::Value;
use smol_str::SmolStr;
use std::io::{self, Read, Write};

// ============================================================================
// Serialization
// ============================================================================

/// Trait for types that can serialize Values into a specific format
pub trait Serializer {
    /// The error type produced by this serializer
    type Error;

    /// Handle serialization of None value
    fn none(&mut self) -> Result<(), Self::Error>;

    /// Handle serialization of integer value
    fn integer(&mut self, value: i32) -> Result<(), Self::Error>;

    /// Handle serialization of string value
    fn string(&mut self, value: &str) -> Result<(), Self::Error>;

    /// Handle serialization of word value
    fn word(&mut self, value: &str) -> Result<(), Self::Error>;

    /// Handle serialization of set-word value
    fn set_word(&mut self, value: &str) -> Result<(), Self::Error>;

    /// Begin serializing a block
    fn begin_block(&mut self, len: usize) -> Result<(), Self::Error>;

    /// End serializing a block
    fn end_block(&mut self) -> Result<(), Self::Error>;
}

/// Extension trait for Value to add serialization capabilities
pub trait ValueSerialize {
    /// Serialize a Value to the given serializer
    fn serialize<S: Serializer>(&self, serializer: &mut S) -> Result<(), S::Error>;
}

impl ValueSerialize for Value {
    fn serialize<S: Serializer>(&self, serializer: &mut S) -> Result<(), S::Error> {
        match self {
            Value::None => serializer.none(),
            Value::Int(n) => serializer.integer(*n),
            Value::String(s) => serializer.string(s),
            Value::Word(w) => serializer.word(w),
            Value::SetWord(w) => serializer.set_word(w),
            Value::Block(block) => {
                serializer.begin_block(block.len())?;
                for item in block.iter() {
                    item.serialize(serializer)?;
                }
                serializer.end_block()
            }
        }
    }
}

/// Error type for binary serialization
#[derive(Debug, thiserror::Error)]
pub enum BinarySerializerError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Serialization error: {0}")]
    SerializeError(String),
}

/// A serializer that writes values in a compact binary format
///
/// Binary format:
/// - Type tag (1 byte)
/// - Length for variable-length data (variable-length encoded integer)
/// - Data (if applicable)
///
/// Type tags are from Tag constants in core.rs:
/// - TAG_NONE (0): None value
/// - TAG_INT (1): Integer (variable-length encoded)
/// - TAG_INLINE_STRING (5): String (length + UTF-8 bytes)
/// - TAG_WORD (6): Word (length + UTF-8 bytes)
/// - TAG_SET_WORD (7): SetWord (length + UTF-8 bytes)
/// - TAG_BLOCK (2): Block (length + contents)
pub struct BinarySerializer<W: Write> {
    writer: W,
}

impl<W: Write> BinarySerializer<W> {
    /// Create a new BinarySerializer with the given writer
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Write a variable-length encoded integer to the writer
    fn write_varint(&mut self, value: i32) -> Result<(), BinarySerializerError> {
        let mut buffer = [0u8; 5];
        let len = encoding::encode_i32(value, &mut buffer)
            .ok_or_else(|| BinarySerializerError::SerializeError("Failed to encode integer".into()))?;
        self.writer.write_all(&buffer[..len])?;
        Ok(())
    }

    /// Write a string with its length prefix
    fn write_string(&mut self, s: &str) -> Result<(), BinarySerializerError> {
        // Write the string length
        self.write_varint(s.len() as i32)?;
        // Write the string data
        self.writer.write_all(s.as_bytes())?;
        Ok(())
    }

    /// Get the writer, consuming the serializer
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> Serializer for BinarySerializer<W> {
    type Error = BinarySerializerError;

    fn none(&mut self) -> Result<(), Self::Error> {
        self.writer.write_all(&[Tag::TAG_NONE as u8])?;
        Ok(())
    }

    fn integer(&mut self, value: i32) -> Result<(), Self::Error> {
        // Write tag
        self.writer.write_all(&[Tag::TAG_INT as u8])?;
        // Write variable-length encoded integer
        self.write_varint(value)
    }

    fn string(&mut self, value: &str) -> Result<(), Self::Error> {
        // Write tag
        self.writer.write_all(&[Tag::TAG_INLINE_STRING as u8])?;
        // Write string with length prefix
        self.write_string(value)
    }

    fn word(&mut self, value: &str) -> Result<(), Self::Error> {
        // Write tag
        self.writer.write_all(&[Tag::TAG_WORD as u8])?;
        // Write string with length prefix
        self.write_string(value)
    }

    fn set_word(&mut self, value: &str) -> Result<(), Self::Error> {
        // Write tag
        self.writer.write_all(&[Tag::TAG_SET_WORD as u8])?;
        // Write string with length prefix
        self.write_string(value)
    }

    fn begin_block(&mut self, len: usize) -> Result<(), Self::Error> {
        // Write tag
        self.writer.write_all(&[Tag::TAG_BLOCK as u8])?;
        // Write length
        self.write_varint(len as i32)
    }

    fn end_block(&mut self) -> Result<(), Self::Error> {
        // No additional data needed for end_block in binary format
        Ok(())
    }
}

/// Serialize a Value to a Vec<u8>
pub fn to_bytes(value: &Value) -> Result<Vec<u8>, BinarySerializerError> {
    let mut serializer = BinarySerializer::new(Vec::new());
    value.serialize(&mut serializer)?;
    Ok(serializer.into_inner())
}

// ============================================================================
// Deserialization
// ============================================================================

/// Error type for binary deserialization
#[derive(Debug, thiserror::Error)]
pub enum BinaryDeserializerError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Deserialization error: {0}")]
    DeserializeError(String),

    #[error("Invalid tag: {0}")]
    InvalidTag(u8),

    #[error("Unexpected end of data")]
    UnexpectedEnd,
}

/// A deserializer that reads values from a binary format
pub struct BinaryDeserializer<R: Read> {
    reader: R,
}

impl<R: Read> BinaryDeserializer<R> {
    /// Create a new BinaryDeserializer with the given reader
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Read a single byte from the reader
    fn read_byte(&mut self) -> Result<u8, BinaryDeserializerError> {
        let mut buf = [0u8; 1];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Read a variable-length encoded integer from the reader
    fn read_varint(&mut self) -> Result<i32, BinaryDeserializerError> {
        let first_byte = self.read_byte()?;
        
        // Determine how many more bytes we need to read
        let additional_bytes = match first_byte {
            // Small values (0-63 for positive, 0x80-0xBF for negative)
            0..=0x3F | 0x80..=0xBF => 0,
            
            // Tag-based values
            0x40..=0x47 => {
                // The number of additional bytes depends on the tag
                match first_byte {
                    0x40 | 0x44 => 1, // 1 additional byte
                    0x41 | 0x45 => 2, // 2 additional bytes
                    0x42 | 0x46 => 3, // 3 additional bytes
                    0x43 | 0x47 => 4, // 4 additional bytes
                    _ => unreachable!(),
                }
            },
            
            // Invalid tag
            _ => return Err(BinaryDeserializerError::InvalidTag(first_byte)),
        };
        
        // Read additional bytes if needed
        let mut buffer = vec![first_byte];
        if additional_bytes > 0 {
            let mut additional = vec![0u8; additional_bytes];
            self.reader.read_exact(&mut additional)?;
            buffer.extend(additional);
        }
        
        // Decode the value
        let (value, _) = encoding::decode_i32(&buffer)
            .ok_or_else(|| BinaryDeserializerError::DeserializeError("Failed to decode integer".into()))?;
            
        Ok(value)
    }

    /// Read a string with its length prefix
    fn read_string(&mut self) -> Result<String, BinaryDeserializerError> {
        // Read the length
        let len = self.read_varint()?;
        if len < 0 {
            return Err(BinaryDeserializerError::DeserializeError("Invalid string length".into()));
        }
        
        // Read the string data
        let mut buffer = vec![0u8; len as usize];
        self.reader.read_exact(&mut buffer)?;
        
        // Convert to UTF-8 string
        String::from_utf8(buffer)
            .map_err(|e| BinaryDeserializerError::DeserializeError(format!("Invalid UTF-8: {}", e)))
    }

    /// Read a single value from the reader
    pub fn read_value(&mut self) -> Result<Value, BinaryDeserializerError> {
        let tag = self.read_byte()?;
        
        match tag {
            t if t == Tag::TAG_NONE as u8 => Ok(Value::None),
            
            t if t == Tag::TAG_INT as u8 => {
                let value = self.read_varint()?;
                Ok(Value::Int(value))
            },
            
            t if t == Tag::TAG_INLINE_STRING as u8 => {
                let value = self.read_string()?;
                Ok(Value::String(SmolStr::new(value)))
            },
            
            t if t == Tag::TAG_WORD as u8 => {
                let value = self.read_string()?;
                Ok(Value::Word(SmolStr::new(value)))
            },
            
            t if t == Tag::TAG_SET_WORD as u8 => {
                let value = self.read_string()?;
                Ok(Value::SetWord(SmolStr::new(value)))
            },
            
            t if t == Tag::TAG_BLOCK as u8 => {
                let len = self.read_varint()?;
                if len < 0 {
                    return Err(BinaryDeserializerError::DeserializeError("Invalid block length".into()));
                }
                
                // Read each value in the block
                let mut values = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    values.push(self.read_value()?);
                }
                
                Ok(Value::Block(values.into_boxed_slice()))
            },
            
            _ => Err(BinaryDeserializerError::InvalidTag(tag)),
        }
    }
}

/// Deserialize a Value from bytes
pub fn from_bytes(bytes: &[u8]) -> Result<Value, BinaryDeserializerError> {
    let mut deserializer = BinaryDeserializer::new(bytes);
    deserializer.read_value()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::parse;
    use std::io::Cursor;

    #[test]
    fn test_serialize_none() {
        let value = Value::None;
        let bytes = to_bytes(&value).unwrap();
        assert_eq!(bytes, vec![Tag::TAG_NONE as u8]);
    }

    #[test]
    fn test_serialize_integer() {
        let value = Value::Int(42);
        let bytes = to_bytes(&value).unwrap();
        // Tag (integer) followed by the varint-encoded 42
        assert_eq!(bytes, vec![Tag::TAG_INT as u8, 42]);

        let value = Value::Int(-1);
        let bytes = to_bytes(&value).unwrap();
        // Tag (integer) followed by the varint-encoded -1
        assert_eq!(bytes, vec![Tag::TAG_INT as u8, 0x80]);
    }

    #[test]
    fn test_serialize_string() {
        let value = Value::String("hello".into());
        let bytes = to_bytes(&value).unwrap();
        // Tag (string), length 5, "hello"
        assert_eq!(bytes, vec![Tag::TAG_INLINE_STRING as u8, 5, b'h', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn test_serialize_word() {
        let value = Value::Word("test".into());
        let bytes = to_bytes(&value).unwrap();
        // Tag (word), length 4, "test"
        assert_eq!(bytes, vec![Tag::TAG_WORD as u8, 4, b't', b'e', b's', b't']);
    }

    #[test]
    fn test_serialize_set_word() {
        let value = Value::SetWord("x".into());
        let bytes = to_bytes(&value).unwrap();
        // Tag (set word), length 1, "x"
        assert_eq!(bytes, vec![Tag::TAG_SET_WORD as u8, 1, b'x']);
    }

    #[test]
    fn test_serialize_empty_block() {
        let value = Value::Block(Box::new([]));
        let bytes = to_bytes(&value).unwrap();
        // Tag (block), length 0
        assert_eq!(bytes, vec![Tag::TAG_BLOCK as u8, 0]);
    }

    #[test]
    fn test_serialize_simple_block() {
        let value = parse("[1 2 3]").unwrap();
        let bytes = to_bytes(&value).unwrap();
        
        // Expected format:
        // - Tag (block)
        // - Length 3
        // - First item: Tag (integer), varint-encoded 1
        // - Second item: Tag (integer), varint-encoded 2
        // - Third item: Tag (integer), varint-encoded 3
        assert_eq!(bytes, vec![
            Tag::TAG_BLOCK as u8, 3, 
            Tag::TAG_INT as u8, 1, 
            Tag::TAG_INT as u8, 2, 
            Tag::TAG_INT as u8, 3
        ]);
    }

    #[test]
    fn test_serialize_complex_value() {
        let value = parse("[\"hello\" world x: 42 [1 2]]").unwrap();
        let bytes = to_bytes(&value).unwrap();
        
        // This complex test primarily verifies that serialization doesn't panic
        // and produces a reasonable output length
        assert!(bytes.len() > 15);
        assert_eq!(bytes[0], Tag::TAG_BLOCK as u8); // Block tag
        assert_eq!(bytes[1], 5); // Length 5 (for 5 items in the block)
    }
    
    #[test]
    fn test_deserialize_none() {
        let bytes = [Tag::TAG_NONE as u8];
        let value = from_bytes(&bytes).unwrap();
        assert!(matches!(value, Value::None));
    }
    
    #[test]
    fn test_deserialize_integer() {
        let bytes = [Tag::TAG_INT as u8, 42];
        let value = from_bytes(&bytes).unwrap();
        assert!(matches!(value, Value::Int(42)));
        
        let bytes = [Tag::TAG_INT as u8, 0x80]; // -1 in varint encoding
        let value = from_bytes(&bytes).unwrap();
        assert!(matches!(value, Value::Int(-1)));
    }
    
    #[test]
    fn test_deserialize_string() {
        let bytes = [Tag::TAG_INLINE_STRING as u8, 5, b'h', b'e', b'l', b'l', b'o'];
        let value = from_bytes(&bytes).unwrap();
        if let Value::String(s) = value {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected String, got {:?}", value);
        }
    }
    
    #[test]
    fn test_deserialize_word() {
        let bytes = [Tag::TAG_WORD as u8, 4, b't', b'e', b's', b't'];
        let value = from_bytes(&bytes).unwrap();
        if let Value::Word(w) = value {
            assert_eq!(w, "test");
        } else {
            panic!("Expected Word, got {:?}", value);
        }
    }
    
    #[test]
    fn test_deserialize_set_word() {
        let bytes = [Tag::TAG_SET_WORD as u8, 1, b'x'];
        let value = from_bytes(&bytes).unwrap();
        if let Value::SetWord(w) = value {
            assert_eq!(w, "x");
        } else {
            panic!("Expected SetWord, got {:?}", value);
        }
    }
    
    #[test]
    fn test_deserialize_empty_block() {
        let bytes = [Tag::TAG_BLOCK as u8, 0];
        let value = from_bytes(&bytes).unwrap();
        if let Value::Block(block) = value {
            assert_eq!(block.len(), 0);
        } else {
            panic!("Expected Block, got {:?}", value);
        }
    }
    
    #[test]
    fn test_deserialize_simple_block() {
        let bytes = [
            Tag::TAG_BLOCK as u8, 3, 
            Tag::TAG_INT as u8, 1, 
            Tag::TAG_INT as u8, 2, 
            Tag::TAG_INT as u8, 3
        ];
        let value = from_bytes(&bytes).unwrap();
        
        if let Value::Block(block) = value {
            assert_eq!(block.len(), 3);
            assert!(matches!(block[0], Value::Int(1)));
            assert!(matches!(block[1], Value::Int(2)));
            assert!(matches!(block[2], Value::Int(3)));
        } else {
            panic!("Expected Block, got {:?}", value);
        }
    }
    
    #[test]
    fn test_roundtrip() {
        // Test roundtrip serialization/deserialization for different value types
        let test_values = vec![
            Value::None,
            Value::Int(42),
            Value::Int(-1),
            Value::Int(100000),
            Value::String("hello".into()),
            Value::Word("test".into()),
            Value::SetWord("x".into()),
            Value::Block(Box::new([])),
            parse("[1 2 3]").unwrap(),
            parse("[\"hello\" world x: 42 [1 2]]").unwrap(),
        ];
        
        for value in test_values {
            let bytes = to_bytes(&value).unwrap();
            let roundtrip = from_bytes(&bytes).unwrap();
            
            assert_eq!(value, roundtrip, "Value did not roundtrip correctly");
        }
    }
    
    #[test]
    fn test_deserialize_invalid_tag() {
        let bytes = [100]; // Invalid tag
        let result = from_bytes(&bytes);
        assert!(matches!(result, Err(BinaryDeserializerError::InvalidTag(100))));
    }
    
    #[test]
    fn test_deserialize_truncated_data() {
        // Truncated integer
        let bytes = [Tag::TAG_INT as u8]; // Tag for integer but no data
        let result = from_bytes(&bytes);
        assert!(matches!(result, Err(BinaryDeserializerError::IoError(_))));
        
        // Truncated string
        let bytes = [Tag::TAG_INLINE_STRING as u8, 5, b'h', b'e']; // Tag for string, length 5, but only 2 chars
        let result = from_bytes(&bytes);
        assert!(matches!(result, Err(BinaryDeserializerError::IoError(_))));
        
        // Truncated block
        let bytes = [Tag::TAG_BLOCK as u8, 3, Tag::TAG_INT as u8, 1]; // Tag for block, length 3, but only one item
        let result = from_bytes(&bytes);
        assert!(matches!(result, Err(BinaryDeserializerError::IoError(_))));
    }
    
    #[test]
    fn test_serialize_to_writer() {
        // Test serializing directly to a writer
        let value = parse("[a: 42 b: \"hello\" c: [1 2 3]]").unwrap();
        let mut buffer = Vec::new();
        
        let mut serializer = BinarySerializer::new(&mut buffer);
        value.serialize(&mut serializer).unwrap();
        
        // Verify the buffer contains the serialized data
        assert!(!buffer.is_empty());
        
        // Deserialize from the buffer and verify
        let deserialized = from_bytes(&buffer).unwrap();
        assert_eq!(value, deserialized);
    }
    
    #[test]
    fn test_deserialize_from_reader() {
        // Create a Value
        let value = parse("[a: 42 b: \"hello\" c: [1 2 3]]").unwrap();
        
        // Serialize to bytes
        let bytes = to_bytes(&value).unwrap();
        
        // Create a reader from the bytes
        let cursor = Cursor::new(bytes);
        
        // Create a deserializer from the reader
        let mut deserializer = BinaryDeserializer::new(cursor);
        
        // Deserialize
        let deserialized = deserializer.read_value().unwrap();
        
        // Verify result
        assert_eq!(value, deserialized);
    }
    
    #[test]
    fn test_serialize_large_nested_structure() {
        // Test with a larger nested structure
        let value = parse(r#"[
            data: [
                [id: 1 name: "Alice"]
                [id: 2 name: "Bob"]
                [id: 3 name: "Charlie"]
            ]
            metadata: [
                created: "2023-01-01"
                version: 1
                settings: [debug: true verbose: false]
            ]
        ]"#).unwrap();
        
        // Serialize and deserialize
        let bytes = to_bytes(&value).unwrap();
        let deserialized = from_bytes(&bytes).unwrap();
        
        // Verify
        assert_eq!(value, deserialized);
    }
}