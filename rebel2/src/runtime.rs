// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Array, Blob, BlobStore, CoreError, Value, WordKind};
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

    fn create_blob(&mut self, data: &[u8]) -> Result<Blob, CoreError> {
        self.blobs.create(data)
    }

    fn get_block_value(&self, blob: &Blob, index: usize) -> Option<Value> {
        match blob {
            Blob::Inline(container) => {
                let data = container
                    .split_first()
                    .and_then(|(len, data)| data.get(..*len as usize))?;
                let block: Array<&[u8], u8> = Array::new(data);
                block.get(index)
            }
            Blob::External(hash) => {
                let data = self.blobs.get(hash).ok()?;
                let block: Array<&[u8], u32> = Array::new(data);
                block.get(index)
            }
        }
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
        let next = self.runtime.get_block_value(&self.block, self.ip)?;
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
        self.module.create_blob(string.as_bytes()).map(|blob| {
            self.parse.push(Value::Block(blob));
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

        let mut block_data = Vec::<u8>::new();
        for value in block_items.iter() {
            value.write(&mut block_data)?;
        }

        let block = Value::Block(self.module.create_blob(&block_data)?);
        Ok(self.parse.push(block))
    }
}
