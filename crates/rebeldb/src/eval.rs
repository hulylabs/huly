// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// eval.rs:

use crate::core::{Content, Hash, Inline, Value};
use anyhow::{Context, Result};
use std::collections::HashMap;

pub fn eval(iter: &mut impl Iterator<Item = Value>) -> Result<Value> {
    let mut value_stack: Vec<Value> = Vec::new();
    let mut operation_stack: Vec<Inline> = Vec::new();
    // let mut env: HashMap<Inline, Value> = HashMap::new();

    for value in iter {
        match value {
            Value::GetWord(word) => match word {
                (5, [b'p', b'r', b'i', b'n', b't', ..]) => {
                    operation_stack.push(word);
                }
                (3, [b'a', b'd', b'd', ..]) => operation_stack.push(word),
                _ => {
                    // let value = env.get(&word).context("word not found")?;
                    value_stack.push(value)
                }
            },
            // let value = env.get(&word).context("word not found").unwrap();
            // value_stack.push(value)
            _ => value_stack.push(value),
        }
    }

    Ok(Value::None)
}
