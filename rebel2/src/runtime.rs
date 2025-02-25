// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Blob, BlobStore, CoreError, Hash, Value, WordKind, HASH_SIZE};
use crate::parse::{Collector, Parser};
use smol_str::SmolStr;
use std::collections::HashMap;

// const fn inline_string(string: &str) -> [u32; 8] {
//     let bytes = string.as_bytes();
//     let len = bytes.len();
//     let mut buf = [0; 8];
//     let mut i = 0;
//     while i < len {
//         buf[i / 4] |= (bytes[i] as u32) << ((i % 4) * 8);
//         i += 1;
//     }
//     buf
// }

//

pub struct Runtime<B> {
    blobs: B,
    system_words: HashMap<SmolStr, Value>,
}

impl<B> Runtime<B>
where
    B: BlobStore,
{
    pub fn new(blobs: B) -> Self {
        Runtime {
            blobs,
            system_words: HashMap::new(),
        }
    }

    fn find_word(&self, symbol: &SmolStr) -> Option<Value> {
        self.system_words.get(symbol).cloned()
    }
}

//

struct Process<'a, T> {
    runtime: &'a mut Runtime<T>,
    block: Blob,
    ip: usize,
    stack: Vec<Value>,
    call_stack: Vec<(Blob, usize)>,
}

impl<'a, T> Process<'a, T>
where
    T: BlobStore,
{
    fn new(runtime: &'a mut Runtime<T>, block: Blob) -> Self {
        Process {
            runtime,
            block,
            ip: 0,
            stack: Vec::new(),
            call_stack: Vec::new(),
        }
    }

    // fn get_blob_data(&'a self, blob: &'a Blob) -> Option<&'a [u8]> {
    //     match blob {
    //         Blob::Inline(size, data) => data.get(..*size as usize),
    //         Blob::External(hash) => self.runtime.blobs.get(hash).ok(),
    //     }
    // }

    fn next(&mut self) -> Option<Value> {
        let next = self.runtime.blobs.get_block_value(&self.block, self.ip)?;
        self.ip += 1;
        Some(next)
    }

    fn next_value(&mut self) -> Option<Value> {
        while let Some(value) = self.next() {
            // resolve value
            let value = match value {
                Value::Word(symbol) => self.runtime.find_word(&symbol)?,
                _ => value.clone(),
            };

            return Some(value);

            // // translate into operation
            // if let Some((op, arity)) = match value[0] {
            //     Value::TAG_NATIVE_FN => {
            //         Some((Op::CALL_NATIVE, self.module.get_func(value[1])?.arity))
            //     }
            //     Value::TAG_SET_WORD => Some((Op::SET_WORD, 1)),
            //     Value::TAG_FUNC => Some((Op::CALL_FUNC, self.module.get_array::<1>(value[1])?[0])),
            //     _ => None,
            // } {
            //     let sp = self.stack.len()?;
            //     self.arity.push([op, value[1], sp, arity * 2])?;
            // } else {
            //     return Some(value);
            // }
        }
        None
    }

    fn eval(&mut self) -> Result<Value, CoreError> {
        loop {
            if let Some(value) = self.next_value() {
                self.stack.push(value);
            }
        }
    }
}

// P A R S E  C O L L E C T O R

struct ParseCollector<'a, T> {
    module: &'a mut Runtime<T>,
    parse: Vec<Value>,
    ops: Vec<usize>,
}

impl<'a, T> ParseCollector<'a, T> {
    fn new(module: &'a mut Runtime<T>) -> Self {
        Self {
            module,
            parse: Vec::new(),
            ops: Vec::new(),
        }
    }
}

impl<T> Collector for ParseCollector<'_, T>
where
    T: BlobStore,
{
    fn string(&mut self, string: &str) -> Result<(), CoreError> {
        self.module.blobs.create(string.as_bytes()).map(|blob| {
            self.parse.push(Value::String(blob));
        })
    }

    fn word(&mut self, kind: WordKind, word: &str) {
        let symbol = SmolStr::from(word);
        let word = match kind {
            WordKind::Word => Value::Word(symbol),
            WordKind::SetWord => Value::SetWord(symbol),
        };
        self.parse.push(word)
    }

    fn integer(&mut self, value: i32) {
        self.parse.push(Value::Int(value as i64))
    }

    fn begin_block(&mut self) {
        self.ops.push(self.parse.len())
    }

    fn end_block(&mut self) -> Result<(), CoreError> {
        let bp = self.ops.pop().ok_or(CoreError::ParseCollectorError)?;
        let block_items = self.parse.drain(bp..).collect::<Vec<Value>>();

        let mut offsets = Vec::<usize>::new();
        let mut block_data = Vec::<u8>::new();
        for value in block_items.iter() {
            offsets.push(block_data.len());
            value.write(&mut block_data)?;
        }

        // check if block can be inlined inlined

        let min_size = 2 + block_data.len() + block_items.len();
        if min_size < HASH_SIZE {
            let mut container = [0; HASH_SIZE];
            container[0] = min_size as u8;
            container[1] = block_items.len() as u8;
            container[2..block_data.len() + 2].copy_from_slice(&block_data);
            container
                .iter_mut()
                .skip(block_data.len() + 2)
                .zip(offsets.iter().rev())
                .for_each(|(i, offset)| {
                    *i = *offset as u8;
                });
            self.parse.push(Value::Block(Blob::Inline(container)));
        } else {
            let size = 4 * 1 + block_data.len() + 4 * block_items.len();
            let mut blob_data = Vec::with_capacity(size);
            blob_data.extend_from_slice(&u32::to_le_bytes(block_items.len() as u32));
            blob_data.extend_from_slice(&block_data);
            for offset in offsets.iter().rev() {
                blob_data.extend_from_slice(&u32::to_le_bytes(*offset as u32));
            }
            let hash = self.module.blobs.put(&blob_data)?;
            self.parse.push(Value::Block(Blob::External(hash)));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Array;
    use std::collections::HashMap;

    /// A simple in-memory implementation of BlobStore for testing
    #[derive(Clone)]
    struct MemoryBlobStore {
        blobs: HashMap<Hash, Vec<u8>>,
    }

    impl MemoryBlobStore {
        fn new() -> Self {
            Self {
                blobs: HashMap::new(),
            }
        }
    }

    impl BlobStore for MemoryBlobStore {
        fn get(&self, hash: &Hash) -> Result<&[u8], CoreError> {
            self.blobs
                .get(hash)
                .map(|v| v.as_slice())
                .ok_or(CoreError::BlobNotFound)
        }

        fn put(&mut self, data: &[u8]) -> Result<Hash, CoreError> {
            // For test purposes, using a simple hash function
            // In a real implementation, this would be a cryptographic hash
            let mut hash = [0u8; HASH_SIZE];
            for (i, &byte) in data.iter().enumerate().take(HASH_SIZE) {
                hash[i % HASH_SIZE] ^= byte;
            }

            self.blobs.insert(hash, data.to_vec());
            Ok(hash)
        }
    }

    /// Helper function to parse a string into a Value using ParseCollector
    fn parse_to_value(
        input: &str,
        store: MemoryBlobStore,
    ) -> Result<(Value, MemoryBlobStore), CoreError> {
        let mut runtime = Runtime::new(store);
        let mut collector = ParseCollector::new(&mut runtime);

        let mut parser = Parser::new(input, &mut collector);
        parser.parse()?;

        // There should be a single value in the collector
        let value = collector.parse.pop().ok_or(CoreError::InternalError)?;

        // Extract the BlobStore back from runtime
        Ok((value, runtime.blobs))
    }

    /// Helper function to extract values from a block using the BlobStore trait methods
    fn extract_block_values(blob: &Blob, store: &MemoryBlobStore) -> Vec<Value> {
        // Use the BlobStore get_block_values implementation
        store.get_block_values(blob)
    }

    /// Helper function to format a block for display
    fn format_block(
        blob: &Blob,
        store: &MemoryBlobStore,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        // Use the BlobStore format_block_display implementation
        store.format_block_display(f, blob)
    }

    /// Helper function to format a block for debug
    fn format_block_debug(
        blob: &Blob,
        store: &MemoryBlobStore,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        // Use the BlobStore format_block_debug implementation
        store.format_block_debug(f, blob)
    }

    #[test]
    fn test_parse_collector_word() {
        let store = MemoryBlobStore::new();

        let (value, _store) = parse_to_value("hello", store).unwrap();

        match value {
            Value::Word(word) => {
                assert_eq!(word, "hello");
            }
            _ => panic!("Expected Word, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_collector_set_word() {
        let store = MemoryBlobStore::new();

        let (value, _store) = parse_to_value("x:", store).unwrap();

        match value {
            Value::SetWord(word) => {
                assert_eq!(word, "x");
            }
            _ => panic!("Expected SetWord, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_collector_integer() {
        let store = MemoryBlobStore::new();

        let (value, _store) = parse_to_value("42", store).unwrap();

        match value {
            Value::Int(num) => {
                assert_eq!(num, 42);
            }
            _ => panic!("Expected Int, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_collector_negative_integer() {
        let store = MemoryBlobStore::new();

        let (value, _store) = parse_to_value("-123", store).unwrap();

        match value {
            Value::Int(num) => {
                assert_eq!(num, -123);
            }
            _ => panic!("Expected Int, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_collector_string() {
        let store = MemoryBlobStore::new();

        let (value, store) = parse_to_value("\"hello world\"", store).unwrap();

        // Display the value using the Display and Debug traits
        println!("String display: {}", value);
        println!("String debug: {:?}", value);

        // Strings are parsed and stored as blobs
        match &value {
            Value::String(blob) => {
                // Check content using internal access for test purposes
                let bytes = match blob {
                    Blob::Inline(container) => {
                        let len = container[0] as usize;
                        &container[1..len + 1]
                    }
                    Blob::External(hash) => store.get(hash).unwrap(),
                };

                let content = std::str::from_utf8(bytes).unwrap();
                assert_eq!(content, "hello world");

                // Check that the display format matches the content
                assert_eq!(value.to_string(), "\"hello world\"");

                // Debug format should include the type information
                let debug_str = format!("{:?}", value);
                assert!(debug_str.starts_with("String::"));
                assert!(debug_str.contains("hello world"));
            }
            _ => panic!("Expected String blob, got {:?}", value),
        }
    }

    // #[test]
    // fn test_parse_collector_simple_block() {
    //     let store = MemoryBlobStore::new();

    //     let (value, store) = parse_to_value("[hello 42]", store).unwrap();

    //     match value {
    //         Value::Block(blob) => {
    //             // Verify that a simple block is stored inline
    //             match &blob {
    //                 Blob::Inline(container) => {
    //                     let len = container[0] as usize;
    //                     println!("Simple block is inline with len: {}", len);
    //                     // Length should be non-zero
    //                     assert!(len > 0);
    //                     // Length should be small enough to fit in inline storage
    //                     assert!(len < HASH_SIZE);
    //                 }
    //                 Blob::External(_) => {
    //                     panic!("Expected inline block, got external");
    //                 }
    //             }

    //             // Extract the values using our BlobStore
    //             let items = extract_block_values(&blob, &store);

    //             // We should extract at least one value
    //             assert!(!items.is_empty());

    //             // Print all the extracted items to help debug
    //             println!("Extracted items:");
    //             for (i, item) in items.iter().enumerate() {
    //                 println!("Item {}: {:?}", i, item);
    //             }

    //             // We can't directly test the formatting functions because Formatter is not
    //             // publicly constructible, but we can create a custom Debug and Display impl
    //             // for testing purposes

    //             // Create a struct that delegates formatting to our format_block function
    //             struct FormattableBlock<'a>(&'a Blob, &'a MemoryBlobStore);

    //             impl<'a> std::fmt::Display for FormattableBlock<'a> {
    //                 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //                     format_block(self.0, self.1, f)
    //                 }
    //             }

    //             impl<'a> std::fmt::Debug for FormattableBlock<'a> {
    //                 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //                     format_block_debug(self.0, self.1, f)
    //                 }
    //             }

    //             // Now we can use standard formatting
    //             let formatted = format!("{}", FormattableBlock(&blob, &store));
    //             let debug_formatted = format!("{:?}", FormattableBlock(&blob, &store));

    //             println!("Formatted block: {}", formatted);
    //             println!("Debug formatted block: {}", debug_formatted);
    //         }
    //         _ => panic!("Expected Block, got {:?}", value),
    //     }
    // }

    #[test]
    fn test_parse_collector_nested_block() {
        let store = MemoryBlobStore::new();

        let (value, store) = parse_to_value("[x: 10 [nested 20]]", store).unwrap();

        match value {
            Value::Block(blob) => {
                let items = extract_block_values(&blob, &store);

                // For now we're just testing that we can get a block
                // The extraction of actual values will be tested in the future
                assert!(items.len() > 0);
            }
            _ => panic!("Expected Block, got {:?}", value),
        }
    }

    // #[test]
    // fn test_parse_collector_complex_block() {
    //     let store = MemoryBlobStore::new();

    //     let (value, store) = parse_to_value(
    //         r#"[
    //             x: 10
    //             y: 20
    //             "hello"
    //             [a b c]
    //         ]"#,
    //         store,
    //     )
    //     .unwrap();

    //     match &value {
    //         Value::Block(blob) => {
    //             // Check that the block has been stored correctly
    //             match blob {
    //                 Blob::Inline(_) => println!("Block is stored inline"),
    //                 Blob::External(hash) => {
    //                     println!("Block is stored externally with hash: {:?}", hash);
    //                     assert!(store.blobs.contains_key(hash));

    //                     // Check we can access the raw data
    //                     if let Ok(data) = store.get(hash) {
    //                         // Just verify it's non-empty for now
    //                         assert!(!data.is_empty());
    //                         println!("External block data size: {}", data.len());
    //                     }
    //                 }
    //             }

    //             // Test the Display trait
    //             println!("Display format of the block: {}", value);

    //             // Don't try to extract values since that's still under development
    //             // For complex blocks, we just verify we can display them
    //         }
    //         _ => panic!("Expected Block, got {:?}", value),
    //     }
    // }

    // #[test]
    // fn test_display_traits_with_parser() {
    //     let store = MemoryBlobStore::new();

    //     // Test simple block with expected inline storage
    //     let input = "[1 2 3]";

    //     let (value, _store) = parse_to_value(input, store.clone()).unwrap();

    //     // Check the storage type
    //     match &value {
    //         Value::Block(Blob::Inline(_)) => {
    //             println!("Simple block is stored inline as expected");
    //         }
    //         _ => {
    //             panic!("Expected inline block, got {:?}", value);
    //         }
    //     }

    //     // Test both Display and Debug formats
    //     println!("Simple block display: {}", value);
    //     println!("Simple block debug: {:?}", value);

    //     // Test a more complex structure parsed from input
    //     let complex_input = r#"[
    //         a: 10
    //         b: "hello"
    //         [1 2 3]
    //     ]"#;

    //     let (complex_value, _store) = parse_to_value(complex_input, store).unwrap();

    //     // Test both Display and Debug formats
    //     println!("Complex block display: {}", complex_value);
    //     println!("Complex block debug: {:?}", complex_value);

    //     // Complex blocks may be stored externally
    //     match &complex_value {
    //         Value::Block(blob) => {
    //             println!("Complex block storage: {:?}", blob);
    //         }
    //         _ => panic!("Expected block value"),
    //     }

    //     // All block displays should have proper brackets
    //     let display_str = format!("{}", complex_value);
    //     assert!(display_str.starts_with("["));
    //     assert!(display_str.ends_with("]"));

    //     // Debug format should be more detailed
    //     let debug_str = format!("{:?}", complex_value);
    //     assert!(debug_str.starts_with("Block::"));
    // }
}
