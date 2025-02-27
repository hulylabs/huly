// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Value, IntoValue, CoreError, inline_string};
use crate::mem::{Context, Heap, Word, Offset};

/// A builder for creating Context objects with a Rust-friendly API
pub struct ContextBuilder<'a, T> {
    heap: &'a mut Heap<T>,
    values: Vec<(String, Value)>,
    size: Offset,
}

impl<'a, T> ContextBuilder<'a, T> 
where
    T: AsMut<[Word]> + AsRef<[Word]>
{
    /// Create a new ContextBuilder with the given heap and size
    pub fn new(heap: &'a mut Heap<T>, size: Offset) -> Self {
        Self {
            heap,
            values: Vec::new(),
            size,
        }
    }
    
    /// Create a new ContextBuilder with the given module's heap and size
    pub fn with_module_heap(heap: &'a mut Heap<T>, size: Offset) -> Self {
        Self {
            heap,
            values: Vec::new(),
            size,
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
    
    /// Add a string value to the context
    pub fn with_string(self, name: &str, value: &str) -> Self {
        self.with_value(name, Value::String(value.to_string()))
    }
    
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
    
    /// Add a word reference to the context
    pub fn with_word(self, name: &str, word: &str) -> Self {
        self.with_value(name, Value::Word(word.to_string()))
    }
    
    /// Add a none/null value to the context
    pub fn with_none(self, name: &str) -> Self {
        self.with_value(name, Value::None)
    }
    
    /// Build the context and return its offset
    pub fn build(self) -> Result<Offset, CoreError> {
        // Create the context
        let ctx_offset = self.heap.alloc_context(self.size).ok_or(CoreError::OutOfMemory)?;
        
        // Make a map from name to VM value (tag, data) to be stored in context
        let mut to_store = Vec::with_capacity(self.values.len());
        
        // Process all values before modifying the context
        for (name, value) in self.values {
            // Get or create the symbol
            let name_inline = inline_string(&name).ok_or(CoreError::StringTooLong)?;
            let symbol = {
                let mut sym_tbl = self.heap.get_symbols_mut().ok_or(CoreError::InternalError)?;
                sym_tbl.get_or_insert(name_inline).ok_or(CoreError::SymbolTableFull)?
            };
            
            // Convert value to VM representation
            let vm_value = value.to_vm_value(self.heap)?;
            
            to_store.push((symbol, vm_value));
        }
        
        // Now store all values in the context
        let ctx_data = self.heap.get_block_mut(ctx_offset).ok_or(CoreError::InternalError)?;
        let mut ctx = Context::new(ctx_data);
        
        for (symbol, value) in to_store {
            ctx.put(symbol, value).ok_or(CoreError::OutOfMemory)?;
        }
        
        Ok(ctx_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Module, Value};
    
    // Create a module to use for testing
    fn setup_module() -> Module<Box<[Word]>> {
        let memory = vec![0; 0x10000].into_boxed_slice();
        Module::init(memory).expect("Failed to initialize module")
    }
    
    #[test]
    fn test_context_builder_generic() {
        // Create a module
        let mut module = setup_module();
        
        // Create a context with the generic builder
        let ctx_offset = {
            let heap = module.get_heap_mut();
            ContextBuilder::new(heap, 10)
                .with("age", 42)
                .with("name", "Test User")
                .with("active", true)
                .with("none", Value::None)
                .build()
                .expect("Failed to build context")
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
        let (block_offset, parent_ctx, ctx_offset) = {
            let heap = module.get_heap_mut();
            
            // Create a block
            let block_offset = heap.alloc_block(&block_data).expect("Failed to create block");
            
            // Create a parent context
            let parent_ctx = ContextBuilder::new(heap, 5)
                .with("value", 100)
                .build()
                .expect("Failed to build parent context");
            
            // Create a context with references using the generic with method
            let ctx_offset = ContextBuilder::new(heap, 10)
                .with("block", BlockOffset(block_offset))
                .with("parent", parent_ctx)  // Offset is treated as Context by default
                .with("ref", WordRef("value".to_string()))
                .build()
                .expect("Failed to build context");
                
            (block_offset, parent_ctx, ctx_offset)
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
            assert_eq!(parent_value[1], parent_ctx);
            
            // Verify word reference
            let ref_value = ctx.get(ref_sym).expect("Failed to get ref");
            assert_eq!(ref_value[0], Value::TAG_WORD);
        }
    }
}