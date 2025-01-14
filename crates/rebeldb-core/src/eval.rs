// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// eval.rs:

use crate::parser::ValueIterator;
use crate::value::{Memory, Value};
use std::result::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    // #[error("word not found: {0:?}")]
    // WordNotFound(Symbol),
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
    #[error("Stack underflow")]
    StackUnderflow,
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

    pub fn push(&mut self, value: Value) -> Result<(), ValueError> {
        if self.sp == 0 {
            Err(ValueError::StackOverflow)
        } else {
            self.sp -= 1;
            self.data[self.sp] = value;
            Ok(())
        }
    }

    pub fn pop(&mut self) -> Result<Value, ValueError> {
        if self.sp < self.size {
            let value = self.data[self.sp];
            self.sp += 1;
            Ok(value)
        } else {
            Err(ValueError::StackUnderflow)
        }
    }
}

//

pub type NativeFn = fn(&mut Stack) -> anyhow::Result<()>;

pub struct Module {
    pub procs: &'static [(&'static str, NativeFn)],
}

pub struct Process<'a, M: Memory> {
    memory: &'a mut M,
    stack: Vec<Value>,
    op_stack: Vec<Value>,
}

impl<'a, M> Process<'a, M>
where
    M: Memory,
{
    pub fn new(memory: &'a mut M) -> Self {
        Self {
            memory,
            stack: Vec::new(),
            op_stack: Vec::new(),
        }
    }

    // pub fn load_module(&mut self, module: &Module) {
    //     let module_id = self.modules.len();
    //     let mut procs: Vec<NativeFn> = Vec::new();

    //     for (id, proc) in module.procs.iter().enumerate() {
    //         procs.push(proc.1);
    //         let native_fn = Value::native_fn(module_id as u16, id as u32);
    //         self.ctx_put(Symbol::new(proc.0).unwrap(), native_fn);
    //     }
    //     self.modules.push(procs);
    // }

    // pub fn push(&mut self, value: Value) {
    //     match value.tag() {
    //         Value::TAG_NATIVE_FN => {
    //             let unboxed = value.as_native_fn().unwrap();
    //             self.op_stack
    //                 .push(self.modules[unboxed.0 as usize][unboxed.1 as usize])
    //         }
    //         _ => self.stack.push(value),
    //     }
    // }

    // pub fn pop(&mut self) -> Option<Value> {
    //     self.stack.pop()
    // }

    // pub fn ctx_put(&mut self, symbol: Symbol, value: Value) {
    //     self.env.insert(symbol, value);
    // }

    pub fn read(&mut self, value: Value) -> Result<(), EvalError> {
        match value.tag() {
            Value::TAG_WORD
                let symbol = value.symbol();
                if let Some(value) = self.memory.get(&word) {
                    self.push(value.clone());
                    Ok(())
                } else {
                    Err(EvalError::WordNotFound(word))
                }
            }

            Value::Word(word) => {
                if let Some(value) = self.env.get(&word) {
                    self.push(value.clone());
                    Ok(())
                } else {
                    Err(EvalError::WordNotFound(word))
                }
            }
            _ => {
                self.push(value);
                Ok(())
            }
        }
    }

    pub fn read_all(&mut self, values: ValueIterator<'_, M>) -> Result<(), EvalError>
    where
        M: Memory,
    {
        for value in values {
            self.read(value?)?;
        }
        Ok(())
    }

    pub fn eval(&mut self) -> Result<Value, EvalError> {
        while let Some(proc) = self.op_stack.pop() {
            proc(&mut self.stack)?;
        }
        Ok(self.stack.pop().unwrap_or(Value::none()))
    }
}

// pub type NativeFn = fn(&mut Vec<Value>) -> Result<(), EvalError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heap::Hash;

    struct NoHeap;

    impl Heap for NoHeap {
        fn put(&mut self, _data: &[u8]) -> Hash {
            unreachable!()
        }
    }

    #[test]
    fn test_read_all_1() -> Result<(), EvalError> {
        let input = "5";
        let mut blobs = NoHeap;
        let iter = ValueIterator::new(input, &mut blobs);

        let mut ctx = Context::new();
        ctx.read_all(iter)?;

        assert!(ctx.stack.len() == 1);
        assert_eq!(ctx.pop().unwrap().as_int(), Some(5));
        Ok(())
    }

    #[test]
    fn test_eval_1() -> Result<(), EvalError> {
        let input = "5";
        let mut blobs = NoHeap;
        let iter = ValueIterator::new(input, &mut blobs);

        let mut ctx = Context::new();
        ctx.read_all(iter)?;
        let result = ctx.eval()?;

        assert_eq!(result.as_int(), Some(5));
        Ok(())
    }

    #[test]
    fn test_proc_1() -> Result<(), EvalError> {
        let mut ctx = Context::new();
        ctx.load_module(&crate::boot::CORE_MODULE);

        let input = "add 7 8";
        let mut blobs = NoHeap;
        let iter = ValueIterator::new(input, &mut blobs);

        ctx.read_all(iter)?;

        let result = ctx.eval()?;
        assert_eq!(result.as_int(), Some(15));

        Ok(())
    }
}
