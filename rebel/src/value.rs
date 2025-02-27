// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use smol_str::SmolStr;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Int(i32),
    Block(Box<[Value]>),
    String(SmolStr),
    Word(SmolStr),
    SetWord(SmolStr),
    /// Context represents key-value pairs in a REBOL-inspired structure
    /// Keys are stored as SmolStr and values can be any Value type
    Context(Box<[(SmolStr, Value)]>),
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
            },
            Value::Context(pairs) => {
                write!(f, "#(")?;
                let mut first = true;
                for (key, value) in pairs.iter() {
                    if !first {
                        write!(f, " ")?;
                    }
                    first = false;
                    write!(f, "{}: {}", key, value)?;
                }
                write!(f, ")")
            }
        }
    }
}
