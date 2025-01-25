// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::value::{Context, Memory, Value};
use std::result::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("mismatched type")]
    MismatchedType,
    #[error("not enough arguments")]
    NotEnoughArgs,
    #[error(transparent)]
    ParseError(#[from] crate::parser::ParseError),
    #[error(transparent)]
    MemoryError(#[from] crate::value::MemoryError),
    #[error("arity mismatch: expecting {0} parameters, provided {1}")]
    ArityMismatch(usize, usize),
    #[error("Stack overflow")]
    StackOverflow,
}

//

pub type NativeFn = fn(&mut Memory) -> anyhow::Result<()>;

pub struct Module {
    pub procs: &'static [(&'static str, NativeFn)],
}

const OP_STACK_SIZE: usize = 256;

pub struct Process<'a, 'b> {
    memory: &'a mut Memory<'b>,
    ops: usize,
    natives: Vec<NativeFn>,
    root_ctx: Context,
    op_stack: [Value; OP_STACK_SIZE],
}

impl<'a, 'b> Process<'a, 'b> {
    pub fn new(memory: &'a mut Memory<'b>) -> Self {
        Self {
            memory,
            op_stack: [Value::NONE; OP_STACK_SIZE],
            ops: 0,
            natives: Vec::new(),
            root_ctx: Context::empty(),
        }
    }

    pub fn load_module(&mut self, module: &Module) -> Result<(), EvalError> {
        for (symbol, proc) in module.procs.iter() {
            let id = self.natives.len();
            self.natives.push(*proc);
            let native_fn = Value::NativeFn(id as u32);
            let symbol = self.memory.get_or_add_symbol(symbol)?;
            self.root_ctx = self.root_ctx.context_put(self.memory, symbol, native_fn)?;
        }
        Ok(())
    }

    pub fn push(&mut self, value: Value) -> Result<(), EvalError> {
        match value.tag() {
            Value::TAG_NATIVE_FN => self.op_stack.push(value),
            _ => self.stack.push(value),
        }
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    pub fn read(&mut self, value: Value) -> Result<(), EvalError> {
        match value.tag() {
            Value::TAG_WORD => self.push(self.root_ctx.context_get(self.memory, value.symbol())),
            _ => self.push(value),
        }
    }

    pub fn read_all(&mut self, values: impl Iterator<Item = Value>) -> Result<(), EvalError> {
        for value in values {
            self.read(value)?;
        }
        Ok(())
    }

    pub fn eval(&mut self) -> anyhow::Result<Value> {
        while let Some(proc) = self.op_stack.pop() {
            match proc.tag() {
                Value::TAG_NATIVE_FN => {
                    let id = proc.wasm_word() as usize;
                    let native_fn = self.natives[id];
                    native_fn(&mut self.stack)?
                }
                _ => unimplemented!(),
            }
        }
        Ok(self.stack.pop().unwrap_or(Value::none()))
    }
}

use crate::parser::ValueIterator;

pub fn run(input: &str) -> anyhow::Result<Value> {
    let mut mem = OwnMemory::new(0x10000, 0x100, 0x1000);
    let iter = ValueIterator::new(input, &mut mem);
    let values: Result<Vec<Value>, _> = iter.collect();

    let mut process = Process::new(&mut mem);
    // process.load_module(&crate::boot::CORE_MODULE)?;
    process.read_all(values?.into_iter())?;
    process.eval()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ValueIterator;

    #[test]
    fn test_read_all_1() -> Result<(), EvalError> {
        let input = "5";

        let mut mem = OwnMemory::new(0x10000, 0x100, 0x1000);
        let iter = ValueIterator::new(input, &mut mem);
        let values: Result<Vec<Value>, _> = iter.collect();

        let mut process = Process::new(&mut mem);
        process.read_all(values?.into_iter())?;

        let value = process.pop().unwrap().as_int()?;
        assert_eq!(value, 5);
        Ok(())
    }

    #[test]
    fn test_proc_1() -> anyhow::Result<()> {
        let input = "add 7 8";

        let mut mem = OwnMemory::new(0x10000, 0x100, 0x1000);
        let iter = ValueIterator::new(input, &mut mem);
        let values: Result<Vec<Value>, _> = iter.collect();

        let mut process = Process::new(&mut mem);
        process.load_module(&crate::boot::CORE_MODULE)?;
        process.read_all(values?.into_iter())?;
        let value = process.eval()?;
        let result = value.as_int()?;

        assert_eq!(result, 15);
        Ok(())
    }
}
