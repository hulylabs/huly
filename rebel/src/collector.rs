// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::CoreError;
use crate::parse::{Collector, WordKind};
use crate::value::Value;
use smol_str::SmolStr;

/// A collector that builds a Value object from parsed input
#[derive(Default)]
pub struct ValueCollector {
    stack: Vec<Vec<Value>>,
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
}

impl Collector for ValueCollector {
    fn string(&mut self, string: &str) -> Option<()> {
        if let Some(current) = self.stack.last_mut() {
            current.push(Value::String(SmolStr::new(string)));
            Some(())
        } else {
            None
        }
    }

    fn word(&mut self, kind: WordKind, word: &str) -> Option<()> {
        if let Some(current) = self.stack.last_mut() {
            match kind {
                WordKind::Word => current.push(Value::Word(SmolStr::new(word))),
                WordKind::SetWord => current.push(Value::SetWord(SmolStr::new(word))),
            };
            Some(())
        } else {
            None
        }
    }

    fn integer(&mut self, value: i32) -> Option<()> {
        if let Some(current) = self.stack.last_mut() {
            current.push(Value::Int(value));
            Some(())
        } else {
            None
        }
    }

    fn begin_block(&mut self) -> Option<()> {
        self.stack.push(Vec::new());
        Some(())
    }

    fn end_block(&mut self) -> Option<()> {
        if self.stack.len() > 1 {
            let block = self.stack.pop()?;
            let parent = self.stack.last_mut()?;
            parent.push(Value::Block(block.into_boxed_slice()));
            Some(())
        } else {
            // Keep the last block in the stack to be returned by into_value
            Some(())
        }
    }
}

/// Parse a string into a Value
pub fn parse(input: &str) -> Result<Value, CoreError> {
    let mut collector = ValueCollector::new();
    collector.begin_block().ok_or(CoreError::ParseCollectorError)?;
    crate::parse::Parser::new(input, &mut collector).parse()?;
    collector.end_block().ok_or(CoreError::ParseCollectorError)?;
    collector.into_value().ok_or(CoreError::InternalError)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_test(input: &str) -> Value {
        parse(input).expect("Failed to parse input")
    }

    #[test]
    fn test_empty_input() {
        let result = parse_test("");
        assert!(matches!(result, Value::Block(block) if block.is_empty()));
    }

    #[test]
    fn test_parse_integer() {
        let result = parse_test("42");
        assert!(matches!(result, Value::Int(42)));
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
    fn test_string_with_spaces() {
        let result = parse_test("\"hello world\"");
        if let Value::String(s) = result {
            assert_eq!(s, "hello world");
        } else {
            panic!("Expected string, got {:?}", result);
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
}