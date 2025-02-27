// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use smol_str::SmolStr;
use std::fmt;

#[derive(Debug, Clone)]
pub enum Value {
    None,
    Int(i32),
    Block(Box<[Value]>),
    String(SmolStr),
    Word(SmolStr),
    SetWord(SmolStr),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => write!(f, "none"),
            Value::Int(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Word(w) => write!(f, "{}", w),
            Value::SetWord(w) => write!(f, "{}:", w),
            Value::Block(block) => {
                write!(f, "[")?;
                let mut first = true;
                for item in block.iter() {
                    if !first {
                        write!(f, " ")?;
                    }
                    first = false;
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
        }
    }
}
