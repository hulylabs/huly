// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::parse::{Collector, ParserError, WordKind};
use crate::value::Value;
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValueCollectorError {
    #[error("unexpected error")]
    UnexpectedError,
    #[error("invalid path")]
    InvalidPath,
}

/// A collector that builds a Value object from parsed input
#[derive(Default)]
pub struct ValueCollector {
    stack: Vec<Vec<Value>>,
    in_path: bool,
}

impl ValueCollector {
    /// Create a new ValueCollector
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the collected Value after parsing
    pub fn into_value(mut self) -> Option<Value> {
        if self.stack.is_empty() {
            Some(Value::None)
        } else {
            let block = self.stack.pop()?;
            if block.len() == 1 {
                // We've already checked that len == 1, so next() will always succeed
                block.into_iter().next()
            } else {
                Some(Value::Block(block.into_boxed_slice()))
            }
        }
    }

    fn push(&mut self, value: Value) -> Result<(), ValueCollectorError> {
        println!("pushing {:?}", value);
        if let Some(current) = self.stack.last_mut() {
            Ok(current.push(value))
        } else {
            Err(ValueCollectorError::UnexpectedError)
        }
    }

    fn pop_block(&mut self) -> Result<Vec<Value>, ValueCollectorError> {
        self.stack.pop().ok_or(ValueCollectorError::UnexpectedError)
    }
}

impl Collector for ValueCollector {
    type Error = ValueCollectorError;

    fn string(&mut self, string: &str) -> Result<(), Self::Error> {
        self.push(Value::String(SmolStr::new(string)))
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Result<(), Self::Error> {
        let symbol = SmolStr::new(word);
        self.push(match kind {
            WordKind::Word => Value::Word(symbol),
            WordKind::SetWord => Value::SetWord(symbol),
        })
    }

    fn integer(&mut self, value: i32) -> Result<(), Self::Error> {
        self.push(Value::Int(value))
    }

    fn begin_block(&mut self) -> Result<(), Self::Error> {
        if self.in_path {
            return Err(ValueCollectorError::InvalidPath);
        }
        Ok(self.stack.push(Vec::new()))
    }

    fn end_block(&mut self) -> Result<(), Self::Error> {
        if self.stack.len() > 1 {
            let block = self.pop_block()?;
            self.push(Value::Block(block.into_boxed_slice()))?;
        }
        Ok(())
    }

    fn begin_path(&mut self) -> Result<(), Self::Error> {
        self.in_path = true;
        Ok(self.stack.push(Vec::new()))
    }

    fn end_path(&mut self) -> Result<(), Self::Error> {
        self.in_path = false;
        if self.stack.len() > 1 {
            let block = self.pop_block()?;
            self.push(Value::Path(block.into_boxed_slice()))?;
        }
        Ok(())
    }
}

/// Parse a string into a Value
pub fn parse(input: &str) -> Result<Value, ParserError<ValueCollectorError>> {
    let mut collector = ValueCollector::new();
    crate::parse::Parser::new(input, &mut collector).parse_block()?;
    collector.into_value().ok_or(ParserError::UnexpectedError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rebel;

    fn parse_test(input: &str) -> Value {
        parse(input).expect("Failed to parse input")
    }

    // Basic parsing tests

    #[test]
    fn test_empty_input() {
        let result = parse_test("");
        assert!(matches!(result, Value::Block(block) if block.is_empty()));
    }

    #[test]
    fn test_whitespace() {
        let result = parse_test("  \t\n  ");
        assert!(matches!(result, Value::Block(block) if block.is_empty()));
    }

    #[test]
    fn test_parse_integer() {
        let result = parse_test("42");
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_parse_negative_integer() {
        let result = parse_test("-42");
        assert!(matches!(result, Value::Int(-42)));
    }

    #[test]
    fn test_parse_string() {
        let result = parse_test("\"hello\"");
        if let Value::String(s) = result {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected string, got {:?}", result);
        }
    }

    #[test]
    fn test_parse_string_with_spaces() {
        let result = parse_test("\"hello world\"");
        if let Value::String(s) = result {
            assert_eq!(s, "hello world");
        } else {
            panic!("Expected string, got {:?}", result);
        }
    }

    #[test]
    fn test_parse_word() {
        let result = parse_test("hello");
        if let Value::Word(s) = result {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected word, got {:?}", result);
        }
    }

    #[test]
    fn test_parse_set_word() {
        let result = parse_test("hello:");
        if let Value::SetWord(s) = result {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected set word, got {:?}", result);
        }
    }

    // Path tests

    #[test]
    fn test_parse_path_1() {
        let result = parse_test("context/name");
        if let Value::Path(path) = result {
            assert_eq!(path.len(), 2);
            assert!(matches!(&path[0], Value::Word(s) if s == "context"));
            assert!(matches!(&path[1], Value::Word(s) if s == "name"));
        } else {
            panic!("Expected path, got {:?}", result);
        }
    }

    #[test]
    fn test_parse_path_2() {
        let result = parse_test("context/name/first");
        if let Value::Path(path) = result {
            assert_eq!(path.len(), 3);
            assert!(matches!(&path[0], Value::Word(s) if s == "context"));
            assert!(matches!(&path[1], Value::Word(s) if s == "name"));
            assert!(matches!(&path[2], Value::Word(s) if s == "first"));
        } else {
            panic!("Expected path, got {:?}", result);
        }
    }

    // Block parsing tests

    #[test]
    fn test_parse_empty_block() {
        let result = parse_test("[]");
        if let Value::Block(block) = result {
            assert_eq!(block.len(), 0);
        } else {
            panic!("Expected block, got {:?}", result);
        }
    }

    #[test]
    fn test_parse_block() {
        let result = parse_test("[1 2 3]");
        if let Value::Block(block) = result {
            assert_eq!(block.len(), 3);
            assert!(matches!(block[0], Value::Int(1)));
            assert!(matches!(block[1], Value::Int(2)));
            assert!(matches!(block[2], Value::Int(3)));
        } else {
            panic!("Expected block, got {:?}", result);
        }
    }

    #[test]
    fn test_nested_blocks() {
        let result = parse_test("[1 [2 3] 4]");
        if let Value::Block(block) = result {
            assert_eq!(block.len(), 3);
            assert!(matches!(block[0], Value::Int(1)));

            if let Value::Block(inner) = &block[1] {
                assert_eq!(inner.len(), 2);
                assert!(matches!(inner[0], Value::Int(2)));
                assert!(matches!(inner[1], Value::Int(3)));
            } else {
                panic!("Expected inner block, got {:?}", block[1]);
            }

            assert!(matches!(block[2], Value::Int(4)));
        } else {
            panic!("Expected block, got {:?}", result);
        }
    }

    #[test]
    fn test_mixed_types() {
        let result = parse_test("[42 \"hello\" world x: [1 2]]");
        if let Value::Block(block) = result {
            assert_eq!(block.len(), 5);

            // Check the integer
            assert!(matches!(block[0], Value::Int(42)));

            // Check the string
            if let Value::String(ref s) = block[1] {
                assert_eq!(s, "hello");
            } else {
                panic!("Expected string, got {:?}", block[1]);
            }

            // Check the word
            if let Value::Word(ref s) = block[2] {
                assert_eq!(s, "world");
            } else {
                panic!("Expected word, got {:?}", block[2]);
            }

            // Check the set word
            if let Value::SetWord(ref s) = block[3] {
                assert_eq!(s, "x");
            } else {
                panic!("Expected set word, got {:?}", block[3]);
            }

            // Check the nested block
            if let Value::Block(ref inner) = block[4] {
                assert_eq!(inner.len(), 2);
                assert!(matches!(inner[0], Value::Int(1)));
                assert!(matches!(inner[1], Value::Int(2)));
            } else {
                panic!("Expected block, got {:?}", block[4]);
            }
        } else {
            panic!("Expected block, got {:?}", result);
        }
    }

    #[test]
    fn test_complex_nested_block() {
        let result = parse_test("[ a: 1 b: 2 c: [d: 3 e: \"hi\"] ]");
        if let Value::Block(block) = result {
            assert_eq!(block.len(), 6);

            // Check a: 1
            if let Value::SetWord(ref s) = block[0] {
                assert_eq!(s, "a");
            } else {
                panic!("Expected set word, got {:?}", block[0]);
            }
            assert!(matches!(block[1], Value::Int(1)));

            // Check b: 2
            if let Value::SetWord(ref s) = block[2] {
                assert_eq!(s, "b");
            } else {
                panic!("Expected set word, got {:?}", block[2]);
            }
            assert!(matches!(block[3], Value::Int(2)));

            // Check c: [d: 3 e: "hi"]
            if let Value::SetWord(ref s) = block[4] {
                assert_eq!(s, "c");
            } else {
                panic!("Expected set word, got {:?}", block[4]);
            }

            if let Value::Block(ref inner) = block[5] {
                assert_eq!(inner.len(), 4);

                // Check d: 3
                if let Value::SetWord(ref s) = inner[0] {
                    assert_eq!(s, "d");
                } else {
                    panic!("Expected set word, got {:?}", inner[0]);
                }
                assert!(matches!(inner[1], Value::Int(3)));

                // Check e: "hi"
                if let Value::SetWord(ref s) = inner[2] {
                    assert_eq!(s, "e");
                } else {
                    panic!("Expected set word, got {:?}", inner[2]);
                }

                if let Value::String(ref s) = inner[3] {
                    assert_eq!(s, "hi");
                } else {
                    panic!("Expected string, got {:?}", inner[3]);
                }
            } else {
                panic!("Expected inner block, got {:?}", block[5]);
            }
        } else {
            panic!("Expected block, got {:?}", result);
        }
    }

    // Note: Tests that require evaluation (like variable assignment, function calls, etc.)
    // are in core.rs. The tests here only verify parsing functionality.

    // Rebel macro tests

    #[test]
    fn test_rebel_macro_none() {
        let v = rebel!(none);
        assert_eq!(v, Value::None);
    }

    #[test]
    fn test_rebel_macro_integers() {
        // Direct integer literals
        let v1: Value = rebel!(42);
        assert_eq!(v1, Value::Int(42));

        // Negative integer
        let v2: Value = rebel!(-42);
        assert_eq!(v2, Value::Int(-42));

        // Integers in a block
        let v3 = rebel!([ 1 2 3 -4 -5 ]);
        if let Value::Block(items) = v3 {
            assert_eq!(items.len(), 5);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::Int(2));
            assert_eq!(items[2], Value::Int(3));
            assert_eq!(items[3], Value::Int(-4));
            assert_eq!(items[4], Value::Int(-5));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_rebel_macro_strings() {
        // String literals
        let v1: Value = rebel!("hello");
        assert_eq!(v1, Value::String("hello".into()));

        // String in a block
        let v2 = rebel!([ "hello" "world" ]);
        if let Value::Block(items) = v2 {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], Value::String("hello".into()));
            assert_eq!(items[1], Value::String("world".into()));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_rebel_macro_words() {
        // Word in a block
        let v1 = rebel!([ alpha beta gamma ]);
        if let Value::Block(items) = v1 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Word("alpha".into()));
            assert_eq!(items[1], Value::Word("beta".into()));
            assert_eq!(items[2], Value::Word("gamma".into()));
        } else {
            panic!("Expected block");
        }

        // SetWord in a block
        let v2 = rebel!([ x: y: z: ]);
        if let Value::Block(items) = v2 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::SetWord("x".into()));
            assert_eq!(items[1], Value::SetWord("y".into()));
            assert_eq!(items[2], Value::SetWord("z".into()));
        } else {
            panic!("Expected block");
        }
    }

    #[test]
    fn test_rebel_macro_blocks() {
        // Empty block
        let v1: Value = rebel!([]);
        if let Value::Block(items) = v1 {
            assert_eq!(items.len(), 0);
        } else {
            panic!("Not a block");
        }

        // Block with integers
        let v2: Value = rebel!([1, 2, 3]);
        if let Value::Block(items) = v2 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::Int(2));
            assert_eq!(items[2], Value::Int(3));
        } else {
            panic!("Not a block");
        }

        // Block with mixed types
        let v3: Value = rebel!([1, "two", none]);
        if let Value::Block(items) = v3 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::String("two".into()));
            assert_eq!(items[2], Value::None);
        } else {
            panic!("Not a block");
        }
    }

    #[test]
    fn test_rebel_macro_contexts() {
        // Empty context
        let v1: Value = rebel!({});
        if let Value::Context(items) = v1 {
            assert_eq!(items.len(), 0);
        } else {
            panic!("Not a context");
        }

        // Context with standard arrow syntax
        let v2: Value = rebel!({ "name" => "John", "age" => 42 });
        if let Value::Context(items) = v2 {
            assert_eq!(items.len(), 2);

            // Find name and age entries
            let name_entry = items
                .iter()
                .find(|(k, _)| k == "name")
                .expect("name not found");
            let age_entry = items
                .iter()
                .find(|(k, _)| k == "age")
                .expect("age not found");

            assert_eq!(name_entry.1, Value::String("John".into()));
            assert_eq!(age_entry.1, Value::Int(42));
        } else {
            panic!("Not a context");
        }

        // Context with identifier keys
        let v3: Value = rebel!({ name => "John", age => 42 });
        if let Value::Context(items) = v3 {
            assert_eq!(items.len(), 2);

            // Find name and age entries
            let name_entry = items
                .iter()
                .find(|(k, _)| k == "name")
                .expect("name not found");
            let age_entry = items
                .iter()
                .find(|(k, _)| k == "age")
                .expect("age not found");

            assert_eq!(name_entry.1, Value::String("John".into()));
            assert_eq!(age_entry.1, Value::Int(42));
        } else {
            panic!("Not a context");
        }
    }

    #[test]
    fn test_rebel_macro_nested_structures() {
        // Nested blocks
        let v1: Value = rebel!([ 1 [2 3] 4 ]);
        if let Value::Block(items) = v1 {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[2], Value::Int(4));

            if let Value::Block(inner) = &items[1] {
                assert_eq!(inner.len(), 2);
                assert_eq!(inner[0], Value::Int(2));
                assert_eq!(inner[1], Value::Int(3));
            } else {
                panic!("Inner item is not a block");
            }
        } else {
            panic!("Not a block");
        }

        // Nested contexts
        let v2: Value = rebel!({
            user => {
                name => "John",
                profile => {
                    age => 42
                }
            }
        });

        if let Value::Context(items) = v2 {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].0, "user");

            if let Value::Context(user) = &items[0].1 {
                assert_eq!(user.len(), 2);

                // Find name and profile entries
                let name_entry = user
                    .iter()
                    .find(|(k, _)| k == "name")
                    .expect("name not found");
                let profile_entry = user
                    .iter()
                    .find(|(k, _)| k == "profile")
                    .expect("profile not found");

                assert_eq!(name_entry.1, Value::String("John".into()));

                if let Value::Context(profile) = &profile_entry.1 {
                    assert_eq!(profile.len(), 1);
                    assert_eq!(profile[0].0, "age");
                    assert_eq!(profile[0].1, Value::Int(42));
                } else {
                    panic!("Profile is not a context");
                }
            } else {
                panic!("User is not a context");
            }
        } else {
            panic!("Not a context");
        }
    }

    #[test]
    fn test_rebel_macro_boolean_values() {
        // Booleans convert to integers (1 and 0)
        let v1: Value = rebel!(true);
        assert_eq!(v1, Value::Int(1));

        let v2: Value = rebel!(false);
        assert_eq!(v2, Value::Int(0));

        // In contexts
        let v3: Value = rebel!({ active => true });
        if let Value::Context(items) = v3 {
            assert_eq!(items[0].1, Value::Int(1));
        } else {
            panic!("Not a context");
        }
    }
}
