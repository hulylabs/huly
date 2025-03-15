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
            let ctx = module.alloc_context(64)?;
            module.push_context(ctx)?;
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
    println!("args: {:?}", args);
    match args {
        [VmValue::TAG_WORD, word, VmValue::TAG_BLOCK, data, VmValue::TAG_BLOCK, body] => {
            if let Ok(value) = module.get_block::<2>(data, 0) {
                let ctx_offset = module.alloc_context(1)?;
                let mut ctx = module.get_context(ctx_offset)?;
                ctx.put(word, value)?;
                ctx.seal()?;

                println!("ctx: {:?}", ctx);

                module.push_context(ctx_offset)?;
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
    let [tag1, params, tag2, body] = module.pop::<4>()?;
    if tag1 != VmValue::TAG_BLOCK || tag2 != VmValue::TAG_BLOCK {
        return Err(CoreError::BadArguments);
    }
    let arity = module.get_block_len(params)? as Offset;
    let func = module.alloc_block(&[
        VmValue::TAG_INT,
        arity,
        VmValue::TAG_BLOCK,
        params,
        VmValue::TAG_BLOCK,
        body,
    ])?;
    module.push([VmValue::TAG_FUNC, func]).map_err(Into::into)
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
    module.pop_to_value().map(|value| print!("{}", value))
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

fn form<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    let value = module.pop_to_value()?;
    let form = value.form();
    module
        .alloc_string(form.as_str())
        .and_then(|s| module.push([VmValue::TAG_INLINE_STRING, s]))
        .map_err(Into::into)
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
    module.add_native_fn("system_print", print, 1)?;
    module.add_native_fn("block?", is_block, 1)?;
    module.add_native_fn("reduce", reduce, 1)?;
    module.add_native_fn("foreach", foreach, 100)?;
    module.add_native_fn("form", form, 1)?;
    Ok(())
}

pub fn test_either(module: &mut Exec<&mut [Word]>) -> Result<(), CoreError> {
    either(module)
}

/// Execute the standard library code for a module
///
/// This function parses and executes the standard library code that defines
/// common functions like print and prin. The stdlib code is read from the
/// stdlib.rebel file at compile time using the include_str! macro.
pub fn stdlib_package<T>(module: &mut Module<T>) -> Result<(), CoreError>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    // Read and execute the standard library
    let stdlib_code = include_str!("stdlib.rebel");
    let vm_block = module.parse(stdlib_code)?;
    module.eval(vm_block)?;

    // Read and execute the extended standard library
    // #[cfg(feature = "stdlib_ext")]
    // {
    //     let stdlib_ext_code = include_str!("stdlib_ext.rebel");
    //     let vm_block_ext = module.parse(stdlib_ext_code)?;
    //     module.eval(vm_block_ext)?;
    // }

    Ok(())
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
