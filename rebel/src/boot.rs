// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Module, Value};
use crate::mem::{Offset, Word};

fn add<T>(module: &mut Module<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]>,
{
    match module.stack_get(bp)? {
        [Value::TAG_INT, a, Value::TAG_INT, b] => {
            let result = a as i32 + b as i32;
            Ok([Value::TAG_INT, result as Word])
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn func_do<T>(module: &mut Module<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.stack_get(bp)? {
        [Value::TAG_BLOCK, b] => {
            let block = module.get_block(b)?;
            let result = module.eval(block.as_ref())?;
            if result.is_empty() {
                Ok([Value::TAG_NONE, 0])
            } else {
                let result = result.last_chunk::<2>().ok_or(CoreError::EndOfInput)?;
                Ok(*result)
            }
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn context<T>(module: &mut Module<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    module.new_context(64)?;
    func_do(module, bp)?;
    let addr = module.pop_context()?;
    Ok([Value::TAG_CONTEXT, addr])
}

fn func<T>(module: &mut Module<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.stack_get(bp)? {
        [Value::TAG_BLOCK, params, Value::TAG_BLOCK, body] => {
            let args = module.get_block(params)?;
            let arg_values = args.len() as Offset / 2;
            let arity = arg_values;
            module.new_context(arity)?;
            for (i, param) in args.as_ref().chunks_exact(2).enumerate() {
                match param {
                    [Value::TAG_WORD, symbol] => module
                        .put_context(*symbol, [Value::TAG_STACK_VALUE, arity - (i as Offset)])?,
                    _ => return Err(CoreError::BadArguments),
                }
            }
            let ctx = module.pop_context()?;
            let func = module.alloc([arg_values, ctx, body])?;
            Ok([Value::TAG_FUNC, func])
        }
        _ => Err(CoreError::BadArguments),
    }
}

pub fn core_package<T>(module: &mut Module<T>) -> Result<(), CoreError>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    module.add_native_fn("add", add, 2)?;
    module.add_native_fn("do", func_do, 1)?;
    module.add_native_fn("context", context, 1)?;
    module.add_native_fn("func", func, 2)?;
    Ok(())
}
