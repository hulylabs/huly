// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// eval.rs:

use crate::core::{Storage, Symbol, Value};
use crate::parser::ValueIterator;
use std::collections::HashMap;
use std::result::Result;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("word not found: {0:?}")]
    WordNotFound(Symbol),
    #[error("unexpected value: {0:?}")]
    UnexpectedValue(Value),
    #[error("not enough arguments")]
    NotEnoughArgs,
    #[error(transparent)]
    ParseError(#[from] crate::parser::ParseError),
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
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    pub fn read(&mut self, value: Value) -> Result<(), EvalError> {
        match value {
            Value::GetWord(word) => {
                if let Some(value) = self.env.get(&word) {
                    self.stack.push(value.clone());
                    Ok(())
                } else {
                    Err(EvalError::WordNotFound(word))
                }
            }
            _ => {
                self.stack.push(value);
                Ok(())
            }
        }
    }

    pub fn read_all<'a, T>(&mut self, values: ValueIterator<'a, T>) -> Result<(), EvalError>
    where
        T: Storage,
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
                    self.stack.push(proc(&args));
                }
                // Value::Block(block) => {
                //     self.op_stack.extend(block.iter().cloned());
                // }
                _ => return Err(EvalError::UnexpectedValue(value)),
            }
        }
        self.stack.pop().ok_or(EvalError::NotEnoughArgs)
    }
}

// pub fn eval(iter: &mut impl Iterator<Item = Value>) -> Result<Value> {
//     let mut value_stack: Vec<Value> = Vec::new();
//     let mut operation_stack: Vec<Inline> = Vec::new();
//     // let mut env: HashMap<Inline, Value> = HashMap::new();

//     for value in iter {
//         match value {
//             Value::GetWord(word) => match word {
//                 (5, [b'p', b'r', b'i', b'n', b't', ..]) => {
//                     operation_stack.push(word);
//                 }
//                 (3, [b'a', b'd', b'd', ..]) => operation_stack.push(word),
//                 _ => {
//                     // let value = env.get(&word).context("word not found")?;
//                     value_stack.push(value)
//                 }
//             },
//             // let value = env.get(&word).context("word not found").unwrap();
//             // value_stack.push(value)
//             _ => value_stack.push(value),
//         }
//     }

//     Ok(Value::None)
// }
