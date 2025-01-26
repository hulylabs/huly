// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::eval::{EvalError, Module};
use crate::value::Value;

fn add(stack: &[u32]) -> anyhow::Result<Value> {
    match stack {
        [Value::INT, a, Value::INT, b, ..] => {
            let result = *a as i32 + *b as i32;
            Ok(result.into())
        }
        _ => Err(EvalError::BadArguments.into()),
    }
}

fn context(stack: &[u32]) -> anyhow::Result<Value> {
    match stack {
        [Value::BLOCK, address, ..] => Ok(Value::NONE),
        _ => Err(EvalError::BadArguments.into()),
    }
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add), ("context", context)],
};
