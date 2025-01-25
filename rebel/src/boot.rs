// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::eval::{EvalError, Module};
use crate::value::{Memory, Value};

fn add(memory: &mut Memory) -> anyhow::Result<()> {
    if let Some(frame) = memory.pop_frame(2) {
        match frame {
            [Value::INT, a, Value::INT, b] => {
                let result = *a as i32 + *b as i32;
                memory.push(result.into())?;
                Ok(())
            }
            _ => Err(EvalError::MismatchedType.into()),
        }
    } else {
        Err(EvalError::StackUnderflow.into())
    }
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add)],
};
