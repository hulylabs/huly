// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::mem::{Context, Heap, MemoryError, Stack, Word};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RebelError {
    #[error(transparent)]
    MemoryError(#[from] MemoryError),
    #[error("function not found")]
    FunctionNotFound,
    #[error("internal error")]
    InternalError,
}

// V A L U E

pub enum Value {
    None,
    Int(i32),
}

impl Value {
    const TAG_NONE: Word = 0;
    const TAG_INT: Word = 1;
    const TAG_WORD: Word = 2;
    const TAG_SET_WORD: Word = 3;
    const TAG_NATIVE_FN: Word = 4;
}

// M O D U L E

type NativeFn<T> =
    for<'a> fn(stack: &[Word], module: &'a mut Module<T>) -> Result<[Word; 2], RebelError>;

struct FuncDesc<T> {
    func: NativeFn<T>,
    arity: u32,
}

pub struct Module<T> {
    heap: Heap<T>,
    functions: Vec<FuncDesc<T>>,
}

impl<T> Module<T> {
    pub fn new(heap: Heap<T>) -> Self {
        Self {
            heap,
            functions: Vec::new(),
        }
    }

    fn get_func(&self, index: u32) -> Result<&FuncDesc<T>, RebelError> {
        self.functions
            .get(index as usize)
            .ok_or(RebelError::FunctionNotFound)
    }
}

impl<T> Module<T>
where
    T: AsMut<[Word]>,
{
    pub fn eval(&mut self, block: &[Word]) -> Result<[Word; 2], RebelError> {
        let mut stack = Stack::new([0; 64]);
        let mut ops = Stack::new([0; 64]);

        let mut cur: Option<[Word; 2]> = None;

        for chunk in block.chunks_exact(2) {
            let value = match chunk[0] {
                Value::TAG_WORD => {
                    let root_ctx = self
                        .heap
                        .get_block_mut(0)
                        .map(Context::new)
                        .ok_or(MemoryError::BoundsCheckFailed)?;
                    root_ctx.get(chunk[1])?
                }
                _ => [chunk[0], chunk[1]],
            };

            let mut sp = stack.alloc(value)?;

            if let Some(arity) = match value[0] {
                Value::TAG_NATIVE_FN => Some(self.get_func(value[1])?.arity * 2),
                Value::TAG_SET_WORD => Some(2),
                _ => None,
            } {
                if let Some(c) = cur {
                    ops.push(c)?;
                }
                cur = Some([sp, arity]);
            }

            while let Some([bp, arity]) = cur {
                if sp == bp + arity {
                    let frame = stack.pop_all(bp)?;
                    match frame {
                        [Value::TAG_SET_WORD, sym, tag, val] => {
                            let mut root_ctx = self
                                .heap
                                .get_block_mut(0)
                                .map(Context::new)
                                .ok_or(MemoryError::BoundsCheckFailed)?;
                            root_ctx.put(*sym, [*tag, *val])?;
                            sp = stack.alloc(value)?;
                        }
                        [Value::TAG_NATIVE_FN, func, ..] => {
                            let native_fn = self.get_func(*func)?;
                            let stack_fn = frame.get(2..).ok_or(MemoryError::BoundsCheckFailed)?;
                            let result = (native_fn.func)(stack_fn, self)?;
                            sp = stack.alloc(result)?;
                        }
                        _ => {
                            return Err(RebelError::InternalError);
                        }
                    }
                    cur = ops.pop();
                } else {
                    break;
                }
            }
        }
        if let Some(value) = stack.pop() {
            Ok(value)
        } else {
            Ok([Value::TAG_NONE, 0])
        }
    }
}

pub fn eval(module: &mut Module<&mut [Word]>, block: &[Word]) -> Result<[Word; 2], RebelError> {
    module.eval(block)
}
