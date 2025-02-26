// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Blob, BlobStore, Block, CoreError, Value, WordKind};
use crate::parse::Collector;
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
    block: Block,
    ip: usize,
    stack: Vec<Value>,
    call_stack: Vec<(Blob, usize)>,
}

impl<'a, T> Process<'a, T>
where
    T: BlobStore,
{
    fn new(runtime: &'a mut Runtime<T>, block: Block) -> Self {
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
        self.block
            .get(&self.runtime.blobs, self.ip)
            .inspect(|_| self.ip += 1)
    }

    // fn next_value(&mut self) -> Option<Value> {
    //     while let Some(value) = self.next() {
    //         // resolve value
    //         let value = match value {
    //             Value::Word(symbol) => self.runtime.find_word(&symbol)?,
    //             _ => value.clone(),
    //         };

    //         return Some(value);

    //         // // translate into operation
    //         // if let Some((op, arity)) = match value[0] {
    //         //     Value::TAG_NATIVE_FN => {
    //         //         Some((Op::CALL_NATIVE, self.module.get_func(value[1])?.arity))
    //         //     }
    //         //     Value::TAG_SET_WORD => Some((Op::SET_WORD, 1)),
    //         //     Value::TAG_FUNC => Some((Op::CALL_FUNC, self.module.get_array::<1>(value[1])?[0])),
    //         //     _ => None,
    //         // } {
    //         //     let sp = self.stack.len()?;
    //         //     self.arity.push([op, value[1], sp, arity * 2])?;
    //         // } else {
    //         //     return Some(value);
    //         // }
    //     }
    //     None
    // }

    // fn eval(&mut self) -> Result<Value, CoreError> {
    //     loop {
    //         if let Some(value) = self.next_value() {
    //             self.stack.push(value);
    //         }
    //     }
    // }
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
        self.module
            .blobs
            .create_blob(string.as_bytes())
            .map(|blob| {
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
        let block_start = self.ops.pop().ok_or(CoreError::InternalError)?;
        let block_items = self.parse.drain(block_start..).collect::<Vec<Value>>();
        let block = Value::new_block(&mut self.module.blobs, &block_items)?;
        self.parse.push(block);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Hash;
    use crate::parse::Parser;
    use std::collections::HashMap;

    const HASH_SIZE: usize = std::mem::size_of::<Hash>();

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
        let value = collector.parse.pop().ok_or(CoreError::InternalError)?;
        Ok((value, runtime.blobs))
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

        assert_eq!(value.to_string(), "\"hello world\"");
    }

    #[test]
    fn test_parse_collector_simple_block() {
        let store = MemoryBlobStore::new();

        let (value, store) = parse_to_value("[hello 42]", store).unwrap();
        assert_eq!(value.to_string(), "[hello 42]");
    }

    #[test]
    fn test_parse_collector_nested_block() {
        let store = MemoryBlobStore::new();

        let (value, store) = parse_to_value("[x: 10 [nested 20]]", store).unwrap();
        println!("value: {}", value);
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
