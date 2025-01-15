//

use crate::eval::{EvalError, Module, Stack};
use crate::value::Value;

fn add(stack: &mut Stack) -> anyhow::Result<()> {
    let b = stack
        .pop()
        .map(Value::as_int)
        .transpose()?
        .ok_or(EvalError::ArityMismatch(2, 0))?;
    let a = stack
        .pop()
        .map(Value::as_int)
        .transpose()?
        .ok_or(EvalError::ArityMismatch(2, 1))?;

    stack.push(Value::new_int(a + b)?)?;
    Ok(())
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add)],
};
