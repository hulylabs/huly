//

use crate::eval::{Module, Stack};
use crate::value::Value;

fn add(stack: &mut Stack) -> anyhow::Result<()> {
    let frame = stack.pop_frame(2)?;
    let result = Value::new_int(frame[1].as_int()? + frame[0].as_int()?)?;
    stack.push(result)?;
    Ok(())
}

pub const CORE_MODULE: Module = Module {
    procs: &[("add", add)],
};
