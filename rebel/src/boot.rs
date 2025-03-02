// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Exec, Module, Op, VmValue};
use crate::mem::{Offset, Word};

fn add<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [VmValue::TAG_INT, a, VmValue::TAG_INT, b] => {
            let result = (a as i32) + (b as i32);
            module
                .push([VmValue::TAG_INT, result as Word])
                .map_err(Into::into)
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn lt<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [VmValue::TAG_INT, a, VmValue::TAG_INT, b] => {
            let result = if (a as i32) < (b as i32) { 1 } else { 0 };
            module.push([VmValue::TAG_BOOL, result]).map_err(Into::into)
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn func_do<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [VmValue::TAG_BLOCK, block] => module.jmp(block),
        _ => Err(CoreError::BadArguments),
    }
}

fn context<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [VmValue::TAG_BLOCK, block] => {
            module.new_context(64)?;
            module.push_op(Op::CONTEXT, 0, 2)?;
            module.jmp(block)
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn func<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [VmValue::TAG_BLOCK, params, VmValue::TAG_BLOCK, body] => {
            let args = module.get_block_len(params)?;
            let arity = args as Offset / 2;
            module.new_context(arity)?;
            for i in 0..arity {
                let param = module.get_block::<2>(params, i * 2)?;
                match param {
                    [VmValue::TAG_WORD, symbol] => {
                        module.put_context(symbol, [VmValue::TAG_STACK_VALUE, 2 * i as Offset])?
                    }
                    _ => return Err(CoreError::BadArguments),
                }
            }
            let ctx = module.pop_context()?;
            let func = module.alloc([arity * 2, ctx, body])?;
            module.push([VmValue::TAG_FUNC, func]).map_err(Into::into)
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn either<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [VmValue::TAG_BOOL, cond, VmValue::TAG_BLOCK, if_true, VmValue::TAG_BLOCK, if_false] => {
            let block = if cond != 0 { if_true } else { if_false };
            module.jmp(block)
        }
        _ => Err(CoreError::BadArguments),
    }
}

fn print<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    let [tag, data] = module.pop()?;
    let vm_value = VmValue::from_tag_data(tag, data)?;
    let value = module.to_value(vm_value)?;
    println!("[print]: {:?}", value);
    Ok(())
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
    module.add_native_fn("print", print, 1)?;
    Ok(())
}

pub fn test_either(module: &mut Exec<&mut [Word]>) -> Result<(), CoreError> {
    either(module)
}
