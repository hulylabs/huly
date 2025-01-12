// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// core.rs:

use crate::eval::{EvalError, Module};
use crate::value::Value;

fn add(stack: &mut Vec<Value>) -> Result<(), EvalError> {
    let b = stack.pop().ok_or(EvalError::ArityMismatch(2, 0))?;
    let a = stack.pop().ok_or(EvalError::ArityMismatch(2, 1))?;

    let result = match (a, b) {
        (Value::I32(a), Value::I32(b)) => Value::I32(a + b),
        (Value::I64(a), Value::I64(b)) => Value::I64(a + b),
        _ => return Err(EvalError::MismatchedType),
    };

    stack.push(result);
    Ok(())
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add)],
};
