// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::{Offset, Word};
use crate::core::SimpleLayout;
use crate::parse::ParseError;
use std::result::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    // #[error("Bad arguments")]
    // BadArguments,
    // #[error("mismatched type")]
    // MismatchedType,
    // #[error(transparent)]
    // ParseError(#[from] crate::parser::ParseError),
    // #[error(transparent)]
    // MemoryError(#[from] crate::value::MemoryError),
    // #[error("Stack overflow")]
    // StackOverflow,
    // #[error("Stack underflow")]
    // StackUnderflow,
    // #[error("Word not found {0}")]
    // WordNotFound(Symbol),
    // #[error("Function not found: {0}")]
    // FunctionNotFound(u32),
    // #[error("Internal error")]
    // InternalError,
    #[error(transparent)]
    NativeError(#[from] anyhow::Error),
}

//

pub type NativeFn = fn(&SimpleLayout) -> Result<(), EvalError>;

pub struct Module {
    pub procs: &'static [(&'static str, NativeFn)],
}

const OP_STACK_SIZE: usize = 256;

pub struct Process<'a> {
    memory: &'a SimpleLayout<'a>,
    natives: Vec<NativeFn>,
}

impl<'a, 'b> Process<'a, 'b> {
    pub fn new(memory: &'a mut MemoryMut<'b>) -> Self {
        Self {
            memory,
            natives: Vec::new(),
        }
    }

    pub fn load_module(&mut self, module: &Module) -> Result<(), EvalError> {
        for (symbol, proc) in module.procs.iter() {
            let id = self.natives.len();
            self.natives.push(*proc);
            let native_fn = Value::native_fn(id as u32);
            let symbol = self.memory.get_or_insert_symbol(symbol)?;
            self.root_ctx = self.root_ctx.add(self.memory, symbol, native_fn)?;
        }
        Ok(())
    }

    fn push_op(&mut self, value: [u32; 3]) -> Result<(), EvalError> {
        if let Some(stack) = self.op_stack.get_mut(self.ops..self.ops + 3) {
            stack[0] = value[0];
            stack[1] = value[1];
            stack[2] = value[2];
            self.ops += 3;
            Ok(())
        } else {
            Err(EvalError::StackOverflow)
        }
    }

    fn pop_op(&mut self) -> Option<&[u32]> {
        if self.ops < 3 {
            None
        } else {
            self.ops -= 3;
            self.op_stack.get(self.ops..self.ops + 3)
        }
    }

    fn push(&mut self, value: Value) -> Result<(), EvalError> {
        match value.tag() {
            Value::NATIVE_FN => self.push_op([
                value.tag(),
                value.payload(),
                self.memory.stack_pointer() as u32,
            ]),
            Value::SET_WORD => self.push_op([
                value.tag(),
                value.payload(),
                self.memory.stack_pointer() as u32,
            ]),
            _ => self.memory.push(value).map_err(EvalError::MemoryError),
        }
    }

    fn read(&mut self, value: Value) -> Result<(), EvalError> {
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
        if let Some(len) = block.len(self.memory) {
            for i in 0..len {
                if let Some(value) = block.get(self.memory, i) {
                    self.read(value)?;
                }
            }
            Ok(())
        } else {
            Err(EvalError::InternalError)
        }
    }

    fn eval_stack(&mut self) -> anyhow::Result<Value> {
        while let Some(op) = self.pop_op() {
            match *op {
                [Value::NATIVE_FN, id, bp] => {
                    if let Some(native_fn) = self.natives.get(id as usize) {
                        native_fn(self.memory, bp as usize)?
                    } else {
                        Err(EvalError::FunctionNotFound(id))?
                    }
                }
                [Value::SET_WORD, sym, bp] => {
                    let value = self
                        .memory
                        .peek(bp as usize)
                        .ok_or(EvalError::StackUnderflow)?;
                    self.root_ctx = self.root_ctx.add(self.memory, sym, value)?;
                }
                _ => return Err(EvalError::InternalError.into()),
            }
        }
        Ok(self.memory.clear().unwrap_or(Value::NONE))
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
    fn test_set_get_word() -> anyhow::Result<()> {
        let mut bytes = vec![0; 0x10000];
        let mut memory = Memory::new(&mut bytes, 0x1000, 0x1000)?;
        let mut process = Process::new(&mut memory);
        process.load_module(&crate::boot::CORE_MODULE)?;

        let value = process.eval("x: 5")?;
        assert_eq!(5 as i32, value.try_into()?);

        let value = process.eval("x")?;
        assert_eq!(5 as i32, value.try_into()?);

        let value = process.eval("add x 2")?;
        assert_eq!(7 as i32, value.try_into()?);

        Ok(())
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
