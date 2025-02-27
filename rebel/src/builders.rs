// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Value, IntoValue, CoreError, inline_string};
use crate::mem::{Context, Heap, Word, Offset};

/// A builder for creating Context objects with a Rust-friendly API
pub struct ContextBuilder<'a, T> {
    heap: &'a mut Heap<T>,
    values: Vec<(String, Value)>,
    capacity: Option<Offset>,
}

impl<'a, T> ContextBuilder<'a, T> 
where
    T: AsMut<[Word]> + AsRef<[Word]>
{
    /// Create a new ContextBuilder with the given heap
    /// 
    /// The capacity will be calculated automatically based on the number of values added.
    pub fn new(heap: &'a mut Heap<T>) -> Self {
        Self {
            heap,
            values: Vec::new(),
            capacity: None,
        }
    }
    
    /// Create a new ContextBuilder with the given heap and explicit capacity
    /// 
    /// Use this if you want to specify a fixed size for the context.
    pub fn with_capacity(heap: &'a mut Heap<T>, capacity: Offset) -> Self {
        Self {
            heap,
            values: Vec::new(),
            capacity: Some(capacity),
        }
    }
    
    /// Create a new ContextBuilder with the given module's heap
    pub fn with_module_heap(heap: &'a mut Heap<T>) -> Self {
        Self {
            heap,
            values: Vec::new(),
            capacity: None,
        }
    }
    
    /// Add a value to the context with the given name
    pub fn with_value(mut self, name: &str, value: Value) -> Self {
        self.values.push((name.to_string(), value));
        self
    }
    
    /// Add a value to the context with the given name using any type that implements IntoValue
    pub fn with<V: IntoValue>(self, name: &str, value: V) -> Self {
        self.with_value(name, value.into_value())
    }
    
    /// Add an integer value to the context
    pub fn with_int(self, name: &str, value: i32) -> Self {
        self.with_value(name, Value::Int(value))
    }
    
    // with_string method has been removed - use module.create_string() and ctx_builder.with() instead
    
    /// Add a boolean value to the context
    pub fn with_bool(self, name: &str, value: bool) -> Self {
        self.with_value(name, Value::Bool(value))
    }
    
    /// Add a context reference to the context
    pub fn with_context(self, name: &str, context: Offset) -> Self {
        self.with_value(name, Value::Context(context))
    }
    
    /// Add a block reference to the context
    pub fn with_block(self, name: &str, block: Offset) -> Self {
        self.with_value(name, Value::Block(block))
    }
    
    // with_word method has been removed - use module.create_word() and ctx_builder.with() instead
    
    /// Add a none/null value to the context
    pub fn with_none(self, name: &str) -> Self {
        self.with_value(name, Value::None)
    }
    
    /// Build the context and return a Value::Context
    pub fn build(self) -> Result<Value, CoreError> {
        // Calculate capacity if not explicitly provided
        // Use the entry count + some padding for growth (25% extra, min 2)
        let capacity = match self.capacity {
            Some(cap) => cap,
            None => {
                let entry_count = self.values.len() as Offset;
                let padding = std::cmp::max(entry_count / 4, 2);
                entry_count + padding
            }
        };
        
        // Create the context
        let ctx_offset = self.heap.alloc_context(capacity).ok_or(CoreError::OutOfMemory)?;
        
        // Convert to a simpler representation of (symbol, [tag, data]) pairs
        let mut to_store = Vec::with_capacity(self.values.len());
        
        // Process all values before modifying the context
        for (name, value) in self.values {
            // Get or create the symbol
            let name_inline = inline_string(&name).ok_or(CoreError::StringTooLong)?;
            let symbol = {
                let mut sym_tbl = self.heap.get_symbols_mut().ok_or(CoreError::InternalError)?;
                sym_tbl.get_or_insert(name_inline).ok_or(CoreError::SymbolTableFull)?
            };
            
            // Convert the value to a VM representation
            let vm_value = match value {
                Value::None => [Value::TAG_NONE, 0],
                Value::Int(i) => [Value::TAG_INT, i as Word],
                Value::Bool(b) => [Value::TAG_BOOL, if b { 1 } else { 0 }],
                Value::Context(c) => [Value::TAG_CONTEXT, c],
                Value::Block(b) => [Value::TAG_BLOCK, b],
                // Handle already processed string values from Module::create_string
                Value::String(offset) => {
                    // Just fetch the [tag, data] pair from the heap
                    self.heap.get(offset).ok_or(CoreError::OutOfMemory)?
                },
                Value::Word(symbol) => {
                    // It's already a proper symbol ID
                    [Value::TAG_WORD, symbol]
                },
                Value::NativeFn(n) => [Value::TAG_NATIVE_FN, n],
                Value::Func(f) => [Value::TAG_FUNC, f],
                Value::SetWord(s) => [Value::TAG_SET_WORD, s],
                Value::StackValue(s) => [Value::TAG_STACK_VALUE, s],
            };
            
            to_store.push((symbol, vm_value));
        }
        
        // Now store all values in the context
        let ctx_data = self.heap.get_block_mut(ctx_offset).ok_or(CoreError::InternalError)?;
        let mut ctx = Context::new(ctx_data);
        
        for (symbol, value) in to_store {
            ctx.put(symbol, value).ok_or(CoreError::OutOfMemory)?;
        }
        
        Ok(Value::Context(ctx_offset))
    }
}

/// A builder for creating Block objects with a Rust-friendly API
pub struct BlockBuilder<'a, T> {
    heap: &'a mut Heap<T>,
    values: Vec<Value>,
}

impl<'a, T> BlockBuilder<'a, T> 
where
    T: AsMut<[Word]> + AsRef<[Word]>
{
    /// Create a new BlockBuilder with the given heap
    pub fn new(heap: &'a mut Heap<T>) -> Self {
        Self {
            heap,
            values: Vec::new(),
        }
    }
    
    /// Add a value to the block
    pub fn with_value(mut self, value: Value) -> Self {
        self.values.push(value);
        self
    }
    
    /// Add a value to the block using any type that implements IntoValue
    pub fn with<V: IntoValue>(self, value: V) -> Self {
        self.with_value(value.into_value())
    }
    
    /// Add an integer value to the block
    pub fn with_int(self, value: i32) -> Self {
        self.with_value(Value::Int(value))
    }
    
    // with_string method has been removed - use module.create_string() and block_builder.with() instead
    
    /// Add a boolean value to the block
    pub fn with_bool(self, value: bool) -> Self {
        self.with_value(Value::Bool(value))
    }
    
    /// Add a none/null value to the block
    pub fn with_none(self) -> Self {
        self.with_value(Value::None)
    }
    
    /// Add a context reference to the block
    pub fn with_context(self, context: Offset) -> Self {
        self.with_value(Value::Context(context))
    }
    
    /// Add a block reference to the block
    pub fn with_block(self, block: Offset) -> Self {
        self.with_value(Value::Block(block))
    }
    
    // with_word method has been removed - use module.create_word() and block_builder.with() instead
    
    /// Build the block and return a Value::Block
    pub fn build(self) -> Result<Value, CoreError> {
        // First, convert each value to its VM representation
        let mut vm_words = Vec::new();
        
        for value in self.values.iter() {
            // Convert the value to a VM representation
            let vm_value = match value {
                Value::None => [Value::TAG_NONE, 0],
                Value::Int(i) => [Value::TAG_INT, *i as Word],
                Value::Bool(b) => [Value::TAG_BOOL, if *b { 1 } else { 0 }],
                Value::Context(c) => [Value::TAG_CONTEXT, *c],
                Value::Block(b) => [Value::TAG_BLOCK, *b],
                // This should never happen now that we don't use special markers
                Value::Word(symbol) if *symbol == u32::MAX => {
                    // This is a legacy/error case - it should never happen in the new API
                    return Err(CoreError::InternalError);
                },
                // Handle already processed string values from Module::create_string
                Value::String(offset) => {
                    // Just fetch the [tag, data] pair from the heap
                    self.heap.get(*offset).ok_or(CoreError::OutOfMemory)?
                },
                Value::Word(symbol) => {
                    // Regular symbol
                    [Value::TAG_WORD, *symbol]
                },
                Value::NativeFn(n) => [Value::TAG_NATIVE_FN, *n],
                Value::Func(f) => [Value::TAG_FUNC, *f],
                Value::SetWord(s) => [Value::TAG_SET_WORD, *s],
                Value::StackValue(s) => [Value::TAG_STACK_VALUE, *s],
            };
            
            vm_words.push(vm_value[0]);
            vm_words.push(vm_value[1]);
        }
        
        // Allocate a block of the right size and store all the values
        let block_offset = self.heap.alloc_block(&vm_words).ok_or(CoreError::OutOfMemory)?;
        
        Ok(Value::Block(block_offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Module;
    use crate::core::Value;
    use crate::BlockOffset;
    
    use crate::MemoryBlobStore;
    
    // Create a module to use for testing
    fn setup_module() -> Module<Box<[Word]>, MemoryBlobStore> {
        let memory = vec![0; 0x10000].into_boxed_slice();
        let blob_store = MemoryBlobStore::new();
        Module::init(memory, blob_store).expect("Failed to initialize module")
    }
    
    #[test]
    fn test_context_builder_generic() {
        // Create a module
        let mut module = setup_module();
        
        // Create a string value first
        let string_value = module.create_string("Test User").expect("Failed to create string");
        
        // Create a context with the generic builder
        let context_value = {
            let heap = module.get_heap_mut();
            ContextBuilder::new(heap)
                .with("age", 42)
                .with("name", string_value)  // Using the string value
                .with("active", true)
                .with("none", Value::None)
                .build()
                .expect("Failed to build context")
        };
        
        // Extract the context offset
        let ctx_offset = match context_value {
            Value::Context(offset) => offset,
            _ => panic!("Expected Context value"),
        };
        
        // Look up symbols to verify values
        let (name_sym, age_sym, active_sym, none_sym) = {
            // Get the name symbol
            let name_sym = {
                let mut sym_tbl = module.get_symbols_mut().expect("Failed to get symbols table");
                sym_tbl.get_or_insert(inline_string("name").unwrap())
                    .expect("Failed to get name symbol")
            };
            
            // Get the age symbol
            let age_sym = {
                let mut sym_tbl = module.get_symbols_mut().expect("Failed to get symbols table");
                sym_tbl.get_or_insert(inline_string("age").unwrap())
                    .expect("Failed to get age symbol")
            };
            
            // Get the active symbol
            let active_sym = {
                let mut sym_tbl = module.get_symbols_mut().expect("Failed to get symbols table");
                sym_tbl.get_or_insert(inline_string("active").unwrap())
                    .expect("Failed to get active symbol")
            };
            
            // Get the none symbol
            let none_sym = {
                let mut sym_tbl = module.get_symbols_mut().expect("Failed to get symbols table");
                sym_tbl.get_or_insert(inline_string("none").unwrap())
                    .expect("Failed to get none symbol")
            };
            
            (name_sym, age_sym, active_sym, none_sym)
        };
        
        // Now verify values in the context
        {
            let heap = module.get_heap_mut();
            let ctx_data = heap.get_block(ctx_offset).expect("Failed to get context block");
            let ctx = Context::new(ctx_data);
            
            // Verify int value
            let age_value = ctx.get(age_sym).expect("Failed to get age");
            assert_eq!(age_value[0], Value::TAG_INT);
            assert_eq!(age_value[1], 42);
            
            // Verify string value type
            let name_value = ctx.get(name_sym).expect("Failed to get name");
            assert_eq!(name_value[0], Value::TAG_INLINE_STRING);
            
            // Verify bool value
            let active_value = ctx.get(active_sym).expect("Failed to get active");
            assert_eq!(active_value[0], Value::TAG_BOOL);
            assert_eq!(active_value[1], 1); // true = 1
            
            // Verify none value
            let none_value = ctx.get(none_sym).expect("Failed to get none");
            assert_eq!(none_value[0], Value::TAG_NONE);
            assert_eq!(none_value[1], 0);
        }
    }
    
    #[test]
    fn test_context_builder_references_generic() {
        // Create a module
        let mut module = setup_module();
        
        // Create a block and contexts
        let block_data = [1, 2, 3, 4, 5];
        let (block_offset, parent_ctx_offset, ctx_offset) = {
            // Create a word value first
            let word_value = module.create_word("value").expect("Failed to create word");
            
            // Now get the heap
            let heap = module.get_heap_mut();
            
            // Create a block
            let block_offset = heap.alloc_block(&block_data).expect("Failed to create block");
            
            // Create a parent context
            let parent_ctx_value = ContextBuilder::new(heap)
                .with("value", 100)
                .build()
                .expect("Failed to build parent context");
                
            let parent_ctx_offset = match parent_ctx_value {
                Value::Context(offset) => offset,
                _ => panic!("Expected Context value"),
            };
            
            // Create a context with references using the generic with method
            let ctx_value = ContextBuilder::new(heap)
                .with("block", BlockOffset(block_offset))
                .with("parent", parent_ctx_offset)  // Offset is treated as Context by default
                .with("ref", word_value)
                .build()
                .expect("Failed to build context");
                
            let ctx_offset = match ctx_value {
                Value::Context(offset) => offset,
                _ => panic!("Expected Context value"),
            };
                
            (block_offset, parent_ctx_offset, ctx_offset)
        };
        
        // Look up symbols to verify values
        let (block_sym, parent_sym, ref_sym) = {
            // Get the block symbol
            let block_sym = {
                let mut sym_tbl = module.get_symbols_mut().expect("Failed to get symbols table");
                sym_tbl.get_or_insert(inline_string("block").unwrap())
                    .expect("Failed to get block symbol")
            };
            
            // Get the parent symbol
            let parent_sym = {
                let mut sym_tbl = module.get_symbols_mut().expect("Failed to get symbols table");
                sym_tbl.get_or_insert(inline_string("parent").unwrap())
                    .expect("Failed to get parent symbol")
            };
            
            // Get the ref symbol
            let ref_sym = {
                let mut sym_tbl = module.get_symbols_mut().expect("Failed to get symbols table");
                sym_tbl.get_or_insert(inline_string("ref").unwrap())
                    .expect("Failed to get ref symbol")
            };
            
            (block_sym, parent_sym, ref_sym)
        };
        
        // Verify all values in the context
        {
            let heap = module.get_heap_mut();
            let ctx_data = heap.get_block(ctx_offset).expect("Failed to get context block");
            let ctx = Context::new(ctx_data);
            
            // Verify block reference
            let block_value = ctx.get(block_sym).expect("Failed to get block");
            assert_eq!(block_value[0], Value::TAG_BLOCK);
            assert_eq!(block_value[1], block_offset);
            
            // Verify parent context reference
            let parent_value = ctx.get(parent_sym).expect("Failed to get parent");
            assert_eq!(parent_value[0], Value::TAG_CONTEXT);
            assert_eq!(parent_value[1], parent_ctx_offset);
            
            // Verify word reference
            let ref_value = ctx.get(ref_sym).expect("Failed to get ref");
            assert_eq!(ref_value[0], Value::TAG_WORD);
        }
    }
    
    #[test]
    fn test_block_builder() {
        // Create a module
        let mut module = setup_module();
        
        // Create a string value first
        let string_value = module.create_string("Hello").expect("Failed to create string");
        
        // Create a block with the block builder
        let block_value = {
            let heap = module.get_heap_mut();
            BlockBuilder::new(heap)
                .with_int(42)
                .with(string_value)  // Using the string value directly
                .with_bool(true)
                .with_none()
                .build()
                .expect("Failed to build block")
        };
        
        // Extract the block offset
        let block_offset = match block_value {
            Value::Block(offset) => offset,
            _ => panic!("Expected Block value"),
        };
        
        // Verify the block contents
        {
            let heap = module.get_heap_mut();
            let block_data = heap.get_block(block_offset).expect("Failed to get block");
            
            // The block should contain 4 values, each with a tag and data (8 words total)
            assert_eq!(block_data.len(), 8);
            
            // Check int value
            assert_eq!(block_data[0], Value::TAG_INT);
            assert_eq!(block_data[1], 42);
            
            // Check string value
            assert_eq!(block_data[2], Value::TAG_INLINE_STRING);
            
            // Check bool value
            assert_eq!(block_data[4], Value::TAG_BOOL);
            assert_eq!(block_data[5], 1); // true = 1
            
            // Check none value
            assert_eq!(block_data[6], Value::TAG_NONE);
            assert_eq!(block_data[7], 0);
        }
    }
    
    #[test]
    fn test_block_builder_nesting() {
        // Create a module
        let mut module = setup_module();
        
        // Create nested blocks and contexts
        let (inner_block_offset, ctx_offset, outer_block_offset) = {
            // Create a string value first
            let inner_string_value = module.create_string("Inner").expect("Failed to create string");
            
            // Create a word value
            let word_value = module.create_word("print").expect("Failed to create word");
            
            // Now get the heap for the builders
            let heap = module.get_heap_mut();
            
            // Create inner block 
            let inner_block_value = BlockBuilder::new(heap)
                .with_int(10)
                .with(inner_string_value)  // Using the pre-created string value
                .build()
                .expect("Failed to build inner block");
                
            let inner_block_offset = match inner_block_value {
                Value::Block(offset) => offset,
                _ => panic!("Expected Block value"),
            };
                
            // Create a context
            let ctx_value = ContextBuilder::new(heap)
                .with("x", 100)
                .with("y", 200)
                .build()
                .expect("Failed to build context");
                
            let ctx_offset = match ctx_value {
                Value::Context(offset) => offset,
                _ => panic!("Expected Context value"),
            };
                
            // Create outer block that references inner structures
            let outer_block_value = BlockBuilder::new(heap)
                .with_int(42)
                .with(inner_block_value)      // Reference to inner block
                .with(ctx_value)              // Reference to context
                .with(word_value)             // Word reference (pre-created)
                .build()
                .expect("Failed to build outer block");
                
            let outer_block_offset = match outer_block_value {
                Value::Block(offset) => offset,
                _ => panic!("Expected Block value"),
            };
                
            (inner_block_offset, ctx_offset, outer_block_offset)
        };
        
        // Verify the outer block contents
        {
            let heap = module.get_heap_mut();
            let block_data = heap.get_block(outer_block_offset).expect("Failed to get outer block");
            
            // The outer block should contain 4 values (8 words)
            assert_eq!(block_data.len(), 8);
            
            // Check int value
            assert_eq!(block_data[0], Value::TAG_INT);
            assert_eq!(block_data[1], 42);
            
            // Check block reference
            assert_eq!(block_data[2], Value::TAG_BLOCK);
            assert_eq!(block_data[3], inner_block_offset);
            
            // Check context reference
            assert_eq!(block_data[4], Value::TAG_CONTEXT);
            assert_eq!(block_data[5], ctx_offset);
            
            // Check word reference
            assert_eq!(block_data[6], Value::TAG_WORD);
        }
    }
}