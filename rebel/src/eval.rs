// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::parser::{parse_block, ParseError};
use crate::value::{Block, Context, Memory, Symbol, Value};
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
    #[error("Word not found")]
    WordNotFound(Symbol),
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

    fn load_module(&mut self, module: &Module) -> Result<(), EvalError> {
        for (symbol, proc) in module.procs.iter() {
            let id = self.natives.len();
            self.natives.push(*proc);
            let native_fn = Value::native_fn(id as u32);
            let symbol = self.memory.get_or_insert_symbol(symbol)?;
            self.root_ctx = self.root_ctx.add(self.memory, symbol, native_fn)?;
        }
        Ok(())
    }

    fn push_op(&mut self, value: Value) -> Result<(), EvalError> {
        if self.ops < OP_STACK_SIZE {
            self.op_stack[self.ops] = value;
            self.ops += 1;
            Ok(())
        } else {
            Err(EvalError::StackOverflow)
        }
    }

    fn pop_op(&mut self) -> Option<Value> {
        if self.ops > 0 {
            self.ops -= 1;
            Some(self.op_stack[self.ops])
        } else {
            None
        }
    }

    fn push(&mut self, value: Value) -> Result<(), EvalError> {
        match value.tag() {
            Value::NATIVE_FN => self.push_op(value),
            _ => self.memory.push(value).map_err(EvalError::MemoryError),
        }
    }

    fn pop(&mut self) -> Option<Value> {
        self.memory.pop()
    }

    pub fn read(&mut self, value: Value) -> Result<(), EvalError> {
        match value.tag() {
            Value::WORD => self.push(
                self.root_ctx
                    .get(self.memory, value.payload())
                    .ok_or(EvalError::WordNotFound(value.payload()))?,
            ),
            _ => self.push(value),
        }
    }

    fn read_block(&mut self, block: Block) -> Result<(), EvalError> {
        let len = block.len(self.memory);
        for i in 0..len {
            self.read(block.get(self.memory, i))?;
        }
        Ok(())
    }

    fn eval_stack(&mut self) -> anyhow::Result<Value> {
        while let Some(proc) = self.pop_op() {
            match proc.tag() {
                Value::NATIVE_FN => {
                    let id = proc.payload() as usize;
                    let native_fn = self.natives[id];
                    native_fn(self.memory)?
                }
                _ => unimplemented!(),
            }
        }
        Ok(self.pop().unwrap_or(Value::NONE))
    }

    pub fn parse(&mut self, input: &str) -> Result<Block, ParseError> {
        parse_block(self.memory, input)
    }

    pub fn eval(&mut self, input: &str) -> anyhow::Result<Value> {
        let block = self.parse(input)?;
        self.read_block(block)?;
        self.eval_stack()
    }
}

pub fn run(input: &str) -> anyhow::Result<Value> {
    let mut bytes = vec![0; 0x10000];
    let mut memory = Memory::new(&mut bytes, 0x1000, 0x1000)?;

    let mut process = Process::new(&mut memory);
    process.load_module(&crate::boot::CORE_MODULE)?;
    process.eval(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn run(input: &str) -> anyhow::Result<Value> {
        let mut bytes = vec![0; 0x10000];
        let mut memory = Memory::new(&mut bytes, 0x1000, 0x1000)?;

        let mut process = Process::new(&mut memory);
        process.load_module(&crate::boot::CORE_MODULE)?;
        process.eval(input)
    }

    #[test]
    fn test_read_all_1() -> anyhow::Result<()> {
        let value = run("5")?;
        assert_eq!(5 as i32, value.try_into()?);
        Ok(())
    }

    #[test]
    fn test_proc_1() -> anyhow::Result<()> {
        let value = run("add 7 8")?;
        assert_eq!(15 as i32, value.try_into()?);
        Ok(())
    }
}
