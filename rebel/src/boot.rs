// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Exec, Module, Op, Value};
use crate::mem::{Offset, Word};

fn add<T>(module: &mut Exec<T>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_INT, a, Value::TAG_INT, b] => {
            let result = (a as i32) + (b as i32);
            module.push([Value::TAG_INT, result as Word])
        }
        _ => None,
    }
}

fn lt<T>(module: &mut Exec<T>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_INT, a, Value::TAG_INT, b] => {
            let result = if (a as i32) < (b as i32) { 1 } else { 0 };
            module.push([Value::TAG_BOOL, result])
        }
        _ => None,
    }
}

fn func_do<T>(module: &mut Exec<T>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_BLOCK, block] => module.call(block),
        _ => None,
    }
}

fn context<T>(module: &mut Exec<T>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_BLOCK, block] => {
            module.new_context(64)?;
            module.push_op(Op::CONTEXT, 0, 2)?;
            module.call(block)
        }
        _ => None,
    }
}

fn func<T>(module: &mut Exec<T>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_BLOCK, params, Value::TAG_BLOCK, body] => {
            let args = module.get_block_len(params)?;
            let arity = args as Offset / 2;
            module.new_context(arity)?;
            for i in 0..arity {
                let param = module.get_block::<2>(params, i * 2)?;
                match param {
                    [Value::TAG_WORD, symbol] => {
                        module.put_context(symbol, [Value::TAG_STACK_VALUE, i as Offset])?
                    }
                    _ => return None,
                }
            }
            let ctx = module.pop_context()?;
            let func = module.alloc([arity, ctx, body])?;
            module.push([Value::TAG_FUNC, func])
        }
        _ => None,
    }
}

fn either<T>(module: &mut Exec<T>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [Value::TAG_BOOL, cond, Value::TAG_BLOCK, if_true, Value::TAG_BLOCK, if_false] => {
            let block = if cond != 0 { if_true } else { if_false };
            module.call(block)
        }
        _ => None,
    }
}

pub fn core_package<T>(module: &mut Module<T>) -> Option<()>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    module.add_native_fn("add", add, 2)?;
    module.add_native_fn("lt", lt, 2)?;
    module.add_native_fn("do", func_do, 1)?;
    module.add_native_fn("context", context, 1)?;
    module.add_native_fn("func", func, 2)?;
    module.add_native_fn("either", either, 3)?;
    Some(())
}
