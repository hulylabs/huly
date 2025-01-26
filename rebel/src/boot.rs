// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::value::{Block, Memory, MemoryError, Module, Value};

fn add(memory: &mut Memory, bp: usize) -> Result<(), MemoryError> {
    match memory.pop_from(bp) {
        Some([Value::INT, a, Value::INT, b, ..]) => {
            let result = *a as i32 + *b as i32;
            memory.push(result.into())?;
            Ok(())
        }
        _ => Err(MemoryError::BadArguments),
    }
}

fn context(memory: &mut Memory, bp: usize) -> Result<(), MemoryError> {
    match memory.pop_from(bp) {
        Some([Value::BLOCK, address, ..]) => {
            let block = Block::new(*address);
            Ok(())
        }
        _ => Err(MemoryError::BadArguments),
    }
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add), ("context", context)],
};
