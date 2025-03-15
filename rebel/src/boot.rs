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

fn reduce<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    match module.pop()? {
        [VmValue::TAG_BLOCK, block] => module.jmp_op(block, Op::REDUCE),
        _ => Err(CoreError::BadArguments),
    }
}

fn foreach<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    let args = module.pop::<6>()?;
    match args {
        [VmValue::TAG_WORD, word, VmValue::TAG_BLOCK, data, VmValue::TAG_BLOCK, body] => {
            if let Ok(value) = module.get_block::<2>(data, 0) {
                module.new_context(1)?;
                module.put_context(word, value)?;
                module.push(args)?;
                module.push([VmValue::TAG_INT, 0])?;
                module.jmp_op(body, Op::FOREACH)
            } else {
                Ok(())
            }
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
    let value = module.pop_to_value()?;
    println!("{}", value);
    Ok(())
}

fn is_block<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    let value = module.pop::<2>()?;
    if value[0] == VmValue::TAG_BLOCK {
        module.push([VmValue::TAG_BOOL, 1]).map_err(Into::into)
    } else {
        module.push([VmValue::TAG_BOOL, 0]).map_err(Into::into)
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
    module.add_native_fn("print", print, 1)?;
    module.add_native_fn("block?", is_block, 1)?;
    module.add_native_fn("reduce", reduce, 1)?;
    module.add_native_fn("foreach", foreach, 3)?;
    Ok(())
}

pub fn test_either(module: &mut Exec<&mut [Word]>) -> Result<(), CoreError> {
    either(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rebel;
    use crate::value::Value;

    #[test]
    fn test_add() {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("can't create module");

        // Create a program [add 7 8] using the rebel! macro
        let program = rebel!([add 7 8]);

        let vm_block = module
            .alloc_value(&program)
            .expect("Failed to allocate block");

        let mut process = module
            .new_process(vm_block)
            .expect("Failed to create process");

        process
            .push_value(Value::int(7))
            .expect("Failed to push value");
        process
            .push_value(Value::int(8))
            .expect("Failed to push value");

        let result = process.eval().expect("Failed to run process");
        let value = module.to_value(result).expect("Failed to get value");

        assert_eq!(value, Value::int(15));
    }
}
