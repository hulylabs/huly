// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Module, Value};
use crate::mem::Word;

fn add<T>(stack: &[Word], _: &mut Module<T>) -> Result<[Word; 2], CoreError> {
    match stack {
        [Value::TAG_INT, a, Value::TAG_INT, b] => {
            let result = *a as i32 + *b as i32;
            Ok([Value::TAG_INT, result as Word])
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn func_do<T>(stack: &[Word], module: &mut Module<T>) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]>,
{
    match stack {
        [Value::TAG_BLOCK, b] => {
            let block = module.get_block(*b)?;

            Ok([Value::TAG_INT, 0])
        }
        _ => Err(CoreError::BadArguments),
    }
}

pub fn core_package<T>(module: &mut Module<T>) -> Result<(), CoreError>
where
    T: AsMut<[Word]>,
{
    module.add_native_fn("add", add, 2)?;
    Ok(())
}
