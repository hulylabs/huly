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
            }
            Value::Context(pairs) => {
                write!(f, "{{ ")?;
                let mut first = true;
                for (key, value) in pairs.iter() {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{}: {}", key, value)?;
                }
                write!(f, " }}")
            }
        }
    }
}

/// Create a None value
#[macro_export]
macro_rules! rebel_none {
    () => {
        crate::value::Value::None
    };
}

/// Create an Int value
#[macro_export]
macro_rules! rebel_int {
    ($i:expr) => {
        crate::value::Value::Int($i)
    };
}

/// Create a String value
#[macro_export]
macro_rules! rebel_string {
    ($s:expr) => {
        crate::value::Value::String($s.into())
    };
}

/// Create a Word value
#[macro_export]
macro_rules! rebel_word {
    ($w:expr) => {
        crate::value::Value::Word($w.into())
    };
}

/// Create a SetWord value
#[macro_export]
macro_rules! rebel_setword {
    ($w:expr) => {
        crate::value::Value::SetWord($w.into())
    };
}

/// Create a Block value
#[macro_export]
macro_rules! rebel_block {
    // Empty block
    () => { crate::value::Value::Block(Box::new([])) };

    // Block with values
    ($($val:expr),+ $(,)?) => {
        crate::value::Value::Block(Box::new([$($val),*]))
    };
}

/// Create a Context value (key-value pairs)
#[macro_export]
macro_rules! rebel_context {
    // Empty context
    () => { crate::value::Value::Context(Box::new([])) };

    // Context with key-value pairs
    ($($key:expr => $val:expr),+ $(,)?) => {
        crate::value::Value::Context(Box::new([$(($key.into(), $val)),*]))
    };
}

/// More expressive form for creating Value types
/// We'll simplify by keeping basic types only
#[macro_export]
macro_rules! rebel {
    // None type
    (none) => {
        crate::value::Value::None
    };

    // Int type
    (int $i:expr) => {
        crate::value::Value::Int($i)
    };

    // String type
    (string $s:expr) => {
        crate::value::Value::String($s.into())
    };

    // Word type
    (word $w:expr) => {
        crate::value::Value::Word($w.into())
    };

    // SetWord type
    (setword $w:expr) => {
        crate::value::Value::SetWord($w.into())
    };

    // Literals (numbers or strings)
    ($i:literal) => {{
        let s = stringify!($i);
        if s.parse::<i32>().is_ok() {
            crate::value::Value::Int($i)
        } else {
            crate::value::Value::String($i.into())
        }
    }};
}

/// Helper function to create Blocks
pub fn block(values: Vec<Value>) -> Value {
    Value::Block(values.into_boxed_slice())
}

/// Helper function to create Context
pub fn context(pairs: Vec<(impl Into<SmolStr>, Value)>) -> Value {
    Value::Context(
        pairs
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

// Example usage
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_macros() {
        // Using the basic macros
        let v1 = rebel_int!(42);
        let v2 = rebel_string!("hello");
        let v3 = rebel_block!(v1.clone(), v2.clone(), rebel_none!());
        let v4 = rebel_context!(
            "name" => rebel_string!("example"),
            "value" => rebel_int!(42)
        );

        assert_eq!(v1, Value::Int(42));
        assert_eq!(v2, Value::String("hello".into()));

        if let Value::Block(items) = v3 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(42));
            assert_eq!(items[1], Value::String("hello".into()));
            assert_eq!(items[2], Value::None);
        } else {
            panic!("v3 is not a Block!");
        }

        if let Value::Context(items) = v4 {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].0, "name");
            assert_eq!(items[0].1, Value::String("example".into()));
            assert_eq!(items[1].0, "value");
            assert_eq!(items[1].1, Value::Int(42));
        } else {
            panic!("v4 is not a Context!");
        }
    }

    #[test]
    fn test_combined_approach() {
        // For complex structures, combine macros and helper functions
        let complex = context(vec![
            ("id", rebel!(int 1)),
            ("name", rebel!(string "example")),
            (
                "tags",
                block(vec![
                    rebel!(string "tag1"),
                    rebel!(string "tag2"),
                    rebel!(int 42),
                ]),
            ),
            (
                "metadata",
                context(vec![
                    ("created", rebel!(string "today")),
                    ("priority", rebel!(int 5)),
                ]),
            ),
        ]);

        if let Value::Context(items) = &complex {
            assert_eq!(items.len(), 4);

            // Check id
            assert_eq!(items[0].0, "id");
            assert_eq!(items[0].1, Value::Int(1));

            // Check name
            assert_eq!(items[1].0, "name");
            assert_eq!(items[1].1, Value::String("example".into()));

            // Check tags
            assert_eq!(items[2].0, "tags");
            if let Value::Block(tags) = &items[2].1 {
                assert_eq!(tags.len(), 3);
                assert_eq!(tags[0], Value::String("tag1".into()));
                assert_eq!(tags[1], Value::String("tag2".into()));
                assert_eq!(tags[2], Value::Int(42));
            } else {
                panic!("tags is not a Block!");
            }

            // Check metadata
            assert_eq!(items[3].0, "metadata");
            if let Value::Context(metadata) = &items[3].1 {
                assert_eq!(metadata.len(), 2);
                assert_eq!(metadata[0].0, "created");
                assert_eq!(metadata[0].1, Value::String("today".into()));
                assert_eq!(metadata[1].0, "priority");
                assert_eq!(metadata[1].1, Value::Int(5));
            } else {
                panic!("metadata is not a Context!");
            }
        } else {
            panic!("complex is not a Context!");
        }
    }
}
