//

use crate::{eval::Stack, value::Value};
use anyhow::Result;

fn add(stack: &mut Stack) -> anyhow::Result<()> {
    let b = stack.pop().map(Value::as_int)?;
    let a = stack.pop().map(Value::as_int)?;

    stack.push(result);
    Ok(())
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add)],
};
