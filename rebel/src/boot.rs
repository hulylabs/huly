// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Exec, Module, Value};
use crate::mem::{Offset, Word};

fn add<T>(module: &mut Exec<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]>,
{
    match module.stack_get(bp)? {
        [Value::TAG_INT, a, Value::TAG_INT, b] => {
            let result = (a as i32) + (b as i32);
            Ok([Value::TAG_INT, result as Word])
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn lt<T>(module: &mut Exec<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]>,
{
    match module.stack_get(bp)? {
        [Value::TAG_INT, a, Value::TAG_INT, b] => {
            let result = (a as i32) < (b as i32);
            Ok([Value::TAG_BOOL, if result { 1 } else { 0 }])
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn func_do<T>(module: &mut Exec<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.stack_get(bp)? {
        [Value::TAG_BLOCK, b] => {
            let block = module.get_block(b)?;
            module.eval(block.as_ref())
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn context<T>(module: &mut Exec<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    module.new_context(64)?;
    func_do(module, bp)?;
    let addr = module.pop_context()?;
    Ok([Value::TAG_CONTEXT, addr])
}

fn func<T>(module: &mut Exec<T>, bp: Offset) -> Result<[Word; 2], CoreError>
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
                    [Value::TAG_WORD, symbol] => {
                        module.put_context(*symbol, [Value::TAG_STACK_VALUE, i as Offset])?
                    }
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

fn either<T>(module: &mut Exec<T>, bp: Offset) -> Result<[Word; 2], CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.stack_get(bp)? {
        [Value::TAG_BOOL, cond, Value::TAG_BLOCK, if_true, Value::TAG_BLOCK, if_false] => {
            let block = if cond != 0 { if_true } else { if_false };
            module.eval(module.get_block(block)?.as_ref())
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
