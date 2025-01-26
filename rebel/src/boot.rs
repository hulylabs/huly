// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::eval::{EvalError, Module};
use crate::value::{Block, Memory, Value};

fn add(memory: &mut Memory, bp: usize) -> anyhow::Result<()> {
    match memory.pop_from(bp) {
        Some([Value::INT, a, Value::INT, b, ..]) => {
            let result = *a as i32 + *b as i32;
            memory.push(result.into())?;
            Ok(())
        }
        _ => Err(EvalError::BadArguments.into()),
    }
}

fn context(memory: &mut Memory, bp: usize) -> anyhow::Result<()> {
    match memory.pop_from(bp) {
        Some([Value::BLOCK, address, ..]) => {
            let block = Block::new(*address);
            Ok(())
        }
        _ => Err(EvalError::BadArguments.into()),
    }
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add), ("context", context)],
};
