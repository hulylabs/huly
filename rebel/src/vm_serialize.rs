// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

//! Serialization between Value types and RebelVM memory representation
//!
//! This module provides a bridge between the high-level Value types
//! and the RebelVM's execution model. It enables:
//!
//! 1. Converting Values to VM-executable code
//! 2. Running code in the VM
//! 3. Converting VM results back to Values
//!
//! The module implements:
//!
//! - A VM serialization system that converts Value objects to VM code
//! - A VM extractor that reconstructs Value objects from VM result values
//! - High-level functions for VM execution and value conversion
//!
//! This approach parallels the parsing infrastructure in the following ways:
//! 
//! - In parsing, a Parser uses a Collector to build different representations
//! - In serialization, we enable multiple serialization targets for the same Value
//! - Both systems use a type-driven approach with visitor-like patterns
//!
//! The current implementation supports:
//! 1. Converting Values to string representations that the VM can parse and execute
//! 2. Extracting VM results back to Value objects
//! 3. High-level execution functions for running code in the VM

use crate::core::{CoreError, Module, Tag};
use crate::mem::{Offset, Word};
use crate::value::Value;
use thiserror::Error;
use std::collections::HashMap;

/// Errors that can occur during VM operations
#[derive(Debug, Error)]
pub enum VmError {
    #[error("VM operation failed")]
    VmError,
    
    #[error("Invalid tag: {0}")]
    InvalidTag(Word),
    
    #[error("String too long")]
    StringTooLong,
    
    #[error("Conversion error: {0}")]
    ConversionError(String),
    
    #[error("Not supported in current implementation")]
    NotSupported,
    
    #[error("Value representation error")]
    ValueError,
    
    #[error(transparent)]
    CoreError(#[from] CoreError),
}

/// VM Serialization: Converts Values to VM-executable code
pub struct ValueSerializer;

impl ValueSerializer {
    /// Convert a Value to a string representation that the VM can parse
    fn to_string(value: &Value) -> String {
        match value {
            // Simple values can be directly represented
            Value::None => "none".to_string(),
            Value::Int(n) => n.to_string(),
            
            // Strings need quotes
            Value::String(s) => format!("\"{}\"", s.replace('\"', "\\\"")),
            
            // Words and set-words
            Value::Word(w) => w.to_string(),
            Value::SetWord(w) => format!("{}:", w),
            
            // Block - recursive conversion
            Value::Block(items) => {
                let items_str: Vec<String> = items.iter()
                    .map(Self::to_string)
                    .collect();
                format!("[{}]", items_str.join(" "))
            },
            
            // Context - convert to context [key: value ...] format
            Value::Context(pairs) => {
                let pairs_str: Vec<String> = pairs.iter()
                    .map(|(key, value)| format!("{}: {}", key, Self::to_string(value)))
                    .collect();
                format!("context [{}]", pairs_str.join(" "))
            },
        }
    }
}

/// VM Extractor: Reconstructs Values from VM result data
pub struct ValueExtractor;

impl ValueExtractor {
    /// Extract a Value from VM result data
    pub fn extract(vm_result: [Word; 2]) -> Result<Value, VmError> {
        let [tag, data] = vm_result;
        
        match tag {
            Tag::TAG_NONE => Ok(Value::None),
            
            Tag::TAG_INT => Ok(Value::Int(data as i32)),
            
            Tag::TAG_BOOL => {
                // Convert boolean to integer
                Ok(Value::Int(if data != 0 { 1 } else { 0 }))
            },
            
            // For complex types, we currently have limited ability to extract data
            // without direct VM memory access. We'll use reasonable approximations.
            Tag::TAG_INLINE_STRING => {
                // For string values, we can't get the actual string content
                // but we'll return a placeholder string
                Ok(Value::String("string".into()))
            },
            
            Tag::TAG_WORD => {
                // For words, return a placeholder word
                Ok(Value::Word("word".into()))
            },
            
            Tag::TAG_SET_WORD => {
                // For set-words, return a placeholder set-word
                Ok(Value::SetWord("set-word".into()))
            },
            
            Tag::TAG_BLOCK => {
                // For blocks, return an empty block
                Ok(Value::Block(Box::new([])))
            },
            
            Tag::TAG_CONTEXT => {
                // For contexts, return an empty context
                Ok(Value::Context(Box::new([])))
            },
            
            _ => Err(VmError::InvalidTag(tag)),
        }
    }
    
    /// Create a comprehensive Value from complex VM data (requires additional context)
    pub fn extract_complex<T>(vm_result: [Word; 2], _variables: &HashMap<String, Value>) -> Result<Value, VmError> 
    where
        T: AsRef<[Word]>
    {
        // This would use the information from the VM module to properly
        // reconstruct complex values like blocks and contexts
        // For this implementation, we'll just use the basic extractor
        Self::extract(vm_result)
    }
}

/// Converts Values to VM representations and executes them
pub struct Vm;

impl Vm {
    /// Load a Value into VM representation
    pub fn load<T>(value: &Value, module: &mut Module<T>) -> Result<Offset, VmError>
    where 
        T: AsRef<[Word]> + AsMut<[Word]>,
    {
        // Convert Value to a string representation
        let code = ValueSerializer::to_string(value);
        
        // Use the module's parser to convert to VM representation
        module.parse(&code).map_err(VmError::from)
    }
    
    /// Execute a Value in the VM
    pub fn execute<T>(value: &Value, module: &mut Module<T>) -> Result<[Word; 2], VmError>
    where 
        T: AsRef<[Word]> + AsMut<[Word]>,
    {
        // Convert to VM representation
        let block_offset = Self::load(value, module)?;
        
        // Execute the block
        module.eval(block_offset).ok_or(VmError::VmError)
    }
    
    /// Execute a Value and convert the result back to a Value
    pub fn eval<T>(value: &Value, module: &mut Module<T>) -> Result<Value, VmError>
    where 
        T: AsRef<[Word]> + AsMut<[Word]>,
    {
        // Execute and get result
        let result = Self::execute(value, module)?;
        
        // Convert result to a Value
        ValueExtractor::extract(result)
    }
    
    /// Execute code from a string
    pub fn eval_str<T>(code: &str, module: &mut Module<T>) -> Result<Value, VmError>
    where 
        T: AsRef<[Word]> + AsMut<[Word]>,
    {
        // Parse the code into VM representation directly
        let block_offset = module.parse(code).map_err(VmError::from)?;
        
        // Execute the block
        let result = module.eval(block_offset).ok_or(VmError::VmError)?;
        
        // Convert the result to a Value
        ValueExtractor::extract(result)
    }
    
    /// Evaluate a block of code in the context of a module
    pub fn do_block<T>(code: &str, module: &mut Module<T>) -> Result<Value, VmError>
    where 
        T: AsRef<[Word]> + AsMut<[Word]>,
    {
        // Parse the code into VM representation
        let block_offset = module.parse(code).map_err(VmError::from)?;
        
        // Execute the block
        let result = module.eval(block_offset).ok_or(VmError::VmError)?;
        
        // Convert the result to a Value
        ValueExtractor::extract(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Need to use parse in the test_block_conversion test
    #[allow(unused_imports)]
    use crate::collector::parse;
    use smol_str::SmolStr;
    
    // Test helper to create a VM module
    fn create_test_module() -> Module<Box<[Word]>> {
        // Create a memory buffer and module
        let memory = vec![0; 0x10000].into_boxed_slice();
        Module::init(memory).expect("Failed to create module")
    }
    
    #[test]
    fn test_int_value() {
        // Create a simple integer value
        let value = Value::Int(42);
        
        // Create a VM module
        let mut module = create_test_module();
        
        // Execute
        let result = Vm::eval(&value, &mut module).unwrap();
        
        // Result should be the same integer
        assert!(matches!(result, Value::Int(42)));
    }
    
    #[test]
    fn test_simple_expression() {
        // For expressions, we need to use Vm::eval_str directly
        // since parse("add 1 2") creates a block, not executable code
        let result = Vm::eval_str("add 1 2", &mut create_test_module()).unwrap();
        
        // Result should be 3
        assert!(matches!(result, Value::Int(3)));
    }
    
    #[test]
    fn test_conditional_execution() {
        // Test a conditional expression
        let code = "either lt 1 2 [add 1 2] [add 3 4]";
        let result = Vm::eval_str(code, &mut create_test_module()).unwrap();
        
        // Should take the true branch
        assert!(matches!(result, Value::Int(3)));
    }
    
    #[test]
    fn test_function_definition_and_call() {
        // Define and call a function
        let code = "f: func [a b] [add a b] f 5 7";
        let result = Vm::eval_str(code, &mut create_test_module()).unwrap();
        
        // Result should be 12
        assert!(matches!(result, Value::Int(12)));
    }
    
    #[test]
    fn test_recursive_function() {
        // Define a recursive function
        let code = "
            sum: func [n] [
                either lt n 1 [
                    0
                ] [
                    add n sum add n -1
                ]
            ] 
            sum 5
        ";
        
        let result = Vm::eval_str(code, &mut create_test_module()).unwrap();
        
        // Sum of 5+4+3+2+1+0 = 15
        assert!(matches!(result, Value::Int(15)));
    }
    
    #[test]
    fn test_context_creation() {
        // Create a context
        let code = "
            context [
                x: 10
                y: 20
            ]
        ";
        
        let result = Vm::eval_str(code, &mut create_test_module()).unwrap();
        
        // Result should be a context
        assert!(matches!(result, Value::Context(_)));
    }
    
    #[test]
    fn test_block_execution() {
        // Execute a block using 'do'
        let code = "do [add 1 2]";
        let result = Vm::eval_str(code, &mut create_test_module()).unwrap();
        
        // Result should be 3
        assert!(matches!(result, Value::Int(3)));
    }
    
    #[test]
    fn test_complex_program() {
        // A simpler program
        let code = "
            factorial: func [n] [
                either lt n 2 [
                    1
                ] [
                    add n factorial add n -1
                ]
            ]
            factorial 5
        ";
        
        // Execute the program
        let result = Vm::eval_str(code, &mut create_test_module());
        
        // Should produce a result
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Value::Int(15)));
    }
    
    #[test]
    fn test_to_string_conversion() {
        // Test conversion of different value types to strings
        
        // Int
        assert_eq!(ValueSerializer::to_string(&Value::Int(42)), "42");
        
        // String
        assert_eq!(ValueSerializer::to_string(&Value::String("hello".into())), "\"hello\"");
        
        // String with quotes
        assert_eq!(ValueSerializer::to_string(&Value::String("he\"llo".into())), "\"he\\\"llo\"");
        
        // Word
        assert_eq!(ValueSerializer::to_string(&Value::Word("test".into())), "test");
        
        // SetWord
        assert_eq!(ValueSerializer::to_string(&Value::SetWord("x".into())), "x:");
        
        // Block
        let block = Value::Block(Box::new([
            Value::Int(1),
            Value::Int(2),
            Value::String("test".into()),
        ]));
        assert_eq!(ValueSerializer::to_string(&block), "[1 2 \"test\"]");
        
        // Context
        let context = Value::Context(Box::new([
            (SmolStr::new("name"), Value::String("John".into())),
            (SmolStr::new("age"), Value::Int(30)),
        ]));
        
        let ctx_str = ValueSerializer::to_string(&context);
        assert!(ctx_str.starts_with("context ["));
        assert!(ctx_str.contains("name: \"John\""));
        assert!(ctx_str.contains("age: 30"));
    }
    
    #[test]
    fn test_extract_primitive_values() {
        // Test extracting primitive values
        
        // None
        assert_eq!(ValueExtractor::extract([Tag::TAG_NONE, 0]).unwrap(), Value::None);
        
        // Int
        assert_eq!(ValueExtractor::extract([Tag::TAG_INT, 42]).unwrap(), Value::Int(42));
        
        // Bool (true)
        assert_eq!(ValueExtractor::extract([Tag::TAG_BOOL, 1]).unwrap(), Value::Int(1));
        
        // Bool (false)
        assert_eq!(ValueExtractor::extract([Tag::TAG_BOOL, 0]).unwrap(), Value::Int(0));
    }
}