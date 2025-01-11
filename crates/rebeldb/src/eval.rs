// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// eval.rs:

use crate::core::{Heap, Symbol, Value};
use crate::parser::ValueIterator;
use std::collections::HashMap;
use std::result::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("word not found: {0:?}")]
    WordNotFound(Symbol),
    #[error("mismatched type: {0:?}")]
    MismatchedType(Value),
    #[error("not enough arguments")]
    NotEnoughArgs,
    #[error(transparent)]
    ParseError(#[from] crate::parser::ParseError),
    #[error(transparent)]
    ValueError(#[from] crate::core::ValueError),
}

pub struct Context {
    stack: Vec<Value>,
    op_stack: Vec<Value>,
    env: HashMap<Symbol, Value>,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            op_stack: Vec::new(),
            env: HashMap::new(),
        }
    }

    pub fn push(&mut self, value: Value) {
        match value {
            Value::NativeFn(_, _) => self.op_stack.push(value),
            _ => self.stack.push(value),
        }
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    pub fn ctx_put(&mut self, symbol: Symbol, value: Value) {
        self.env.insert(symbol, value);
    }

    pub fn read(&mut self, value: Value) -> Result<(), EvalError> {
        match value {
            Value::Word(word) => {
                if let Some(value) = self.env.get(&word) {
                    Ok(self.push(value.clone()))
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

    pub fn read_all<'a, T>(&mut self, values: ValueIterator<'a, T>) -> Result<(), EvalError>
    where
        T: Heap,
    {
        for value in values {
            self.read(value?)?;
        }
        Ok(())
    }

    pub fn eval(&mut self) -> Result<Value, EvalError> {
        while let Some(value) = self.op_stack.pop() {
            match value {
                Value::NativeFn(proc, arity) => {
                    let args = self.stack.split_off(self.stack.len() - arity);
                    self.stack.push(proc(&args)?);
                }
                // Value::Block(block) => {
                //     self.op_stack.extend(block.iter().cloned());
                // }
                _ => return Err(EvalError::MismatchedType(value)),
            }
        }
        Ok(self.stack.pop().unwrap_or(Value::None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Hash;

    struct NoHeap;

    impl Heap for NoHeap {
        fn put(&mut self, _data: &[u8]) -> Hash {
            unreachable!()
        }
    }

    fn add(stack: &Vec<Value>) -> Result<Value, EvalError> {
        let b = &stack[1];
        let a = &stack[0];
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Uint(a), Value::Uint(b)) => Ok(Value::Uint(a + b)),
            _ => Err(EvalError::MismatchedType(b.clone())),
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
        assert_eq!(ctx.pop().unwrap().as_int()?, 5);
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

        assert!(result.as_int()? == 5);
        Ok(())
    }

    #[test]
    fn test_proc_1() -> Result<(), EvalError> {
        let input = "add 7 8";
        let mut blobs = NoHeap;
        let iter = ValueIterator::new(input, &mut blobs);

        let mut ctx = Context::new();
        ctx.ctx_put(Value::new_symbol("add")?, Value::NativeFn(add, 2));
        ctx.read_all(iter)?;

        let result = ctx.eval()?;
        assert!(result.as_int()? == 15);

        Ok(())
    }
}
