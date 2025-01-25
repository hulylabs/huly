// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// eval.rs:

use crate::value::{Memory, Value};
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
    ValueError(#[from] crate::value::ValueError),
    #[error("arity mismatch: expecting {0} parameters, provided {1}")]
    ArityMismatch(usize, usize),
    #[error("Stack overflow")]
    StackOverflow,
}

pub struct Stack {
    data: Box<[Value]>,
    sp: usize,
    size: usize,
}

impl Stack {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![Value::none(); size].into_boxed_slice(),
            sp: size,
            size,
        }
    }

    pub fn push(&mut self, value: Value) -> Result<(), EvalError> {
        if self.sp == 0 {
            Err(EvalError::StackOverflow)
        } else {
            self.sp -= 1;
            self.data[self.sp] = value;
            Ok(())
        }
    }

    pub fn pop(&mut self) -> Option<Value> {
        if self.sp < self.size {
            let value = self.data[self.sp];
            self.sp += 1;
            Some(value)
        } else {
            None
        }
    }

    pub fn pop_frame(&mut self, size: usize) -> Result<&[Value], EvalError> {
        if self.sp + size > self.size {
            Err(EvalError::NotEnoughArgs)
        } else {
            let frame = &self.data[self.sp..self.sp + size];
            self.sp += size;
            Ok(frame)
        }
        // <[Value; N]>::try_from(&self.data[self.sp..self.sp + N])
    }
}

//

pub type NativeFn = fn(&mut Stack) -> anyhow::Result<()>;

pub struct Module {
    pub procs: &'static [(&'static str, NativeFn)],
}

pub struct Process<'a, M: Memory> {
    memory: &'a mut M,
    stack: Stack,
    op_stack: Stack,
    natives: Vec<NativeFn>,
    root_ctx: Value,
}

impl<'a, M> Process<'a, M>
where
    M: Memory,
{
    pub fn new(memory: &'a mut M) -> Self {
        Self {
            memory,
            stack: Stack::new(4096),
            op_stack: Stack::new(256),
            natives: Vec::new(),
            root_ctx: Value::context(),
        }
    }

    pub fn load_module(&mut self, module: &Module) -> Result<(), EvalError> {
        for (symbol, proc) in module.procs.iter() {
            let id = self.natives.len();
            self.natives.push(*proc);
            let native_fn = Value::native_fn(id as u32);
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

    pub fn read_all(&mut self, values: impl Iterator<Item = Value>) -> Result<(), EvalError>
    where
        M: Memory,
    {
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
use crate::value::OwnMemory;

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
    use crate::value::OwnMemory;

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
