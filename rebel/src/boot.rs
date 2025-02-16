// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Exec, Module, Op, Value};
use crate::mem::{Offset, Word};

fn add<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_INT, a, Value::TAG_INT, b] => {
            let result = (a as i32) + (b as i32);
            module.push([Value::TAG_INT, result as Word])
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn lt<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_INT, a, Value::TAG_INT, b] => {
            let result = if (a as i32) < (b as i32) { 1 } else { 0 };
            module.push([Value::TAG_BOOL, result])
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn func_do<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_BLOCK, block] => module.call(block),
        _ => Err(CoreError::BadArguments),
    }
}

fn context<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_BLOCK, block] => {
            module.new_context(64)?;
            module.push_op(Op::CONTEXT, 0, 2)?;
            module.call(block)
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn func<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_BLOCK, params, Value::TAG_BLOCK, body] => {
            let args = module.get_block(params)?;
            let arg_values = args.len() as Offset / 2;
            let arity = arg_values;
            module.new_context(arity)?;
            for (i, param) in args.as_ref().chunks_exact(2).enumerate() {
                match param {
                    [Value::TAG_WORD, symbol] => {
                        module.put_context(*symbol, [Value::TAG_STACK_VALUE, i as Offset])?
                    }
                    _ => return Err(CoreError::BadArguments),
                }
            }
            let ctx = module.pop_context()?;
            let func = module.alloc([arg_values, ctx, body])?;
            module.push([Value::TAG_FUNC, func])
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn either<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_BOOL, cond, Value::TAG_BLOCK, if_true, Value::TAG_BLOCK, if_false] => {
            let block = if cond != 0 { if_true } else { if_false };
            module.call(block)
        }
        _ => Err(CoreError::BadArguments),
    }
}

pub fn core_package<T>(module: &mut Module<T>) -> Result<(), CoreError>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    module.add_native_fn("add", add, 2)?;
    module.add_native_fn("lt", lt, 2)?;
    module.add_native_fn("do", func_do, 1)?;
    module.add_native_fn("context", context, 1)?;
    module.add_native_fn("func", func, 2)?;
    module.add_native_fn("either", either, 3)?;
    Ok(())
}
