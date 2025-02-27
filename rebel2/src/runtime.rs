// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Blob, BlobStore, Block, CoreError, Value, Values, WordKind};
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

// R U N T I M E

type NativeFn = fn(process: &mut Process) -> Result<(), CoreError>;

struct FuncDesc {
    func: NativeFn,
    arity: usize,
}

pub struct Runtime {
    blobs: Box<dyn BlobStore>,
    functions: Vec<FuncDesc>,
    system_words: HashMap<SmolStr, Value>,
}

impl Runtime {
    pub fn new(blobs: Box<dyn BlobStore>) -> Self {
        Runtime {
            blobs,
            functions: Vec::new(),
            system_words: HashMap::new(),
        }
    }

    fn get_func(&self, index: usize) -> Result<&FuncDesc, CoreError> {
        self.functions
            .get(index)
            .ok_or(CoreError::InvalidNativeFunction)
    }

    fn find_word(&self, symbol: &SmolStr) -> Option<&Value> {
        self.system_words.get(symbol)
    }
}

//

enum Op {
    Value(Value),
    SetWord(SmolStr),
    CallNative(usize),
}

struct Process<'a> {
    runtime: &'a mut Runtime,
    // block: Block,
    // ip: usize,
    stack: Vec<Value>,
    call_stack: Vec<(Block, usize)>,
    op_stack: Vec<(Op, usize, usize)>,
    base: Vec<usize>,
}

impl<'a> Process<'a> {
    fn new(runtime: &'a mut Runtime, block: Block) -> Result<Self, CoreError> {
        Ok(Process {
            runtime,
            // block,
            // ip: 0,
            stack: Vec::new(),
            call_stack: Vec::new(),
            op_stack: Vec::new(),
            base: Vec::new(),
        })
    }

    fn do_op(&mut self, op: &Op) -> Result<(), CoreError> {
        match op {
            Op::SetWord(symbol) => self
                .stack
                .pop()
                .and_then(|value| {
                    self.runtime
                        .system_words
                        .insert(symbol.clone(), value.clone())
                        .map(|_| ())
                })
                .ok_or(CoreError::InternalError),
            Op::CallNative(index) => {
                let native = self.runtime.get_func(*index)?;
                (native.func)(self)
            }
            Op::Value(value) => Ok(self.stack.push(value.clone())),
        }
    }

    fn operation(&self, value: Value) -> Result<(Op, usize, usize), CoreError> {
        let stack_len = self.stack.len();
        match value {
            Value::NativeFunc(index) => self
                .runtime
                .get_func(index)
                .map(|native| (Op::CallNative(index), native.arity, stack_len)),
            Value::SetWord(symbol) => Ok((Op::SetWord(symbol.clone()), 1, stack_len)),
            // Value::TAG_FUNC => Some((Op::CALL_FUNC, self.module.get_array::<1>(value[1])?[0])),
            _ => Ok((Op::Value(value), 0, stack_len)),
        }
    }

    fn resolve(&self, value: Value) -> Result<Value, CoreError> {
        if let Value::Word(symbol) = value {
            self.runtime
                .find_word(&symbol)
                .and_then(|bound| match bound {
                    Value::StackRef(offset) => {
                        self.base.last().map(|bp| self.stack[*bp + *offset].clone())
                    }
                    _ => Some(bound.clone()),
                })
                .ok_or(CoreError::WordNotFound)
        } else {
            Ok(value)
        }
    }

    fn read_next(&mut self) -> Result<Value, CoreError> {
        let ip = self.call_stack.last_mut().ok_or(CoreError::EndOfInput)?;
        ip.0.get(self.runtime.blobs.as_ref(), ip.1)
            .inspect(|_| ip.1 += 1)
    }

    fn next(&mut self) -> Result<(), CoreError> {
        if let Some(op) = self.op_stack.last() {
            if self.stack.len() == op.1 + op.2 {
                let (op, _, _) = self.op_stack.pop().ok_or(CoreError::InternalError)?;
                return self.do_op(&op);
            }
        }
        let op = self
            .read_next()
            .and_then(|value| self.resolve(value))
            .and_then(|value| self.operation(value))?;
        Ok(self.op_stack.push(op))
    }

    // fn eval(&mut self) -> Result<Value, CoreError> {
    //     loop {
    //         if let Some(value) = self.next_value() {
    //             self.stack.push(value);
    //         }
    //     }
    // }
}

pub fn next(process: &mut Process) -> Result<(), CoreError> {
    process.next()
}

// P A R S E  C O L L E C T O R

struct ParseCollector<'a> {
    module: &'a mut Runtime,
    parse: Vec<Value>,
    ops: Vec<usize>,
}

impl<'a> ParseCollector<'a> {
    fn new(module: &'a mut Runtime) -> Self {
        Self {
            module,
            parse: Vec::new(),
            ops: Vec::new(),
        }
    }
}

impl Collector for ParseCollector<'_> {
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
        let block = Value::new_block(self.module.blobs.as_mut(), &block_items)?;
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
            let mut hash = [0u8; HASH_SIZE];
            for (i, &byte) in data.iter().enumerate().take(HASH_SIZE) {
                hash[i % HASH_SIZE] ^= byte;
            }

            self.blobs.insert(hash, data.to_vec());
            Ok(hash)
        }
    }

    /// Helper function to parse a string into a Value using ParseCollector
    fn parse_to_value(input: &str) -> Result<Value, CoreError> {
        let store = MemoryBlobStore::new();
        let mut runtime = Runtime::new(Box::new(store));
        let mut collector = ParseCollector::new(&mut runtime);
        let mut parser = Parser::new(input, &mut collector);

        parser.parse()?;
        let value = collector.parse.pop().ok_or(CoreError::InternalError)?;
        Ok(value)
    }

    #[test]
    fn test_parse_collector_word() {
        let value = parse_to_value("hello").unwrap();

        match value {
            Value::Word(word) => {
                assert_eq!(word, "hello");
            }
            _ => panic!("Expected Word, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_collector_set_word() {
        let value = parse_to_value("x:").unwrap();

        match value {
            Value::SetWord(word) => {
                assert_eq!(word, "x");
            }
            _ => panic!("Expected SetWord, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_collector_integer() {
        let value = parse_to_value("42").unwrap();

        match value {
            Value::Int(num) => {
                assert_eq!(num, 42);
            }
            _ => panic!("Expected Int, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_collector_negative_integer() {
        let value = parse_to_value("-123").unwrap();

        match value {
            Value::Int(num) => {
                assert_eq!(num, -123);
            }
            _ => panic!("Expected Int, got {:?}", value),
        }
    }

    #[test]
    fn test_parse_collector_string() {
        let value = parse_to_value("\"hello world\"").unwrap();

        assert_eq!(value.to_string(), "\"hello world\"");
    }

    #[test]
    fn test_parse_collector_simple_block() {
        let value = parse_to_value("[hello 42]").unwrap();
        assert_eq!(value.to_string(), "[hello 42]");
    }

    #[test]
    fn test_parse_collector_nested_block() {
        let value = parse_to_value("[x: 10 [nested 20]]").unwrap();
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
