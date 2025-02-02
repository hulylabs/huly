// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use super::Word;
use crate::core::{Block, Module, RebelError, TAG_INT};

// fn(stack: &[Word], heap: Block<&mut [Word]>) -> Result<(), RebelError>

fn add(stack: &[Word], _: Block<&mut [Word]>) -> Result<[Word; 2], RebelError> {
    match stack {
        [TAG_INT, a, TAG_INT, b, ..] => {
            let result = *a as i32 + *b as i32;
            Ok([TAG_INT, result as Word])
        }
        _ => Err(RebelError::BadArguments),
    }
}

// fn context(memory: &mut Memory, bp: usize) -> Result<(), MemoryError> {
//     match memory.pop_from(bp) {
//         Some([Value::BLOCK, address, ..]) => {
//             let block = Block::new(*address);
//             Ok(())
//         }
//         _ => Err(MemoryError::BadArguments),
//     }
// }

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add, 2)],
};
