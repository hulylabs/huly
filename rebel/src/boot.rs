// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::eval::{EvalError, Module};
use crate::value::Value;

fn add(stack: &[u32]) -> anyhow::Result<Value> {
    if stack.len() != 4 {
        return Err(EvalError::NotEnoughArgs.into());
    }
    match stack {
        [Value::INT, a, Value::INT, b] => {
            let result = *a as i32 + *b as i32;
            Ok(result.into())
        }
        _ => Err(EvalError::MismatchedType.into()),
    }
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add)],
};
