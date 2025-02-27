// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::boot::core_package;
use crate::core::{inline_string, CoreError, Value};
use crate::mem::{Context, Heap, Offset, SymbolTable, Word};

// FuncDesc struct moved from core.rs
use crate::core::NativeFn;

/// A function descriptor for native functions
pub struct FuncDesc<T, B> {
    pub func: NativeFn<T, B>,
    pub arity: u32,
}

pub type Hash = [u8; 32];

pub trait BlobStore {
    fn get(&self, hash: &Hash) -> Result<&[u8], CoreError>;
    fn put(&mut self, data: &[u8]) -> Result<Hash, CoreError>;
}

/// Module struct that serves as the main interface to the RebelDB VM
pub struct Module<T, B> {
    store: B,
    heap: Heap<T>,
    system_words: Offset,
    functions: Vec<FuncDesc<T, B>>,
}

impl<T, B> Module<T, B> {
    // const NULL: Offset = 0;
    const SYMBOLS: Offset = 1;
    // const CONTEXT: Offset = 2;
}

impl<T, B> Module<T, B>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
    B: BlobStore,
{
    pub fn init(data: T, store: B) -> Option<Self> {
        let mut heap = Heap::new(data);
        heap.init(3)?;

        let system_words = heap.alloc_context(1024)?;

        let mut module = Self {
            store,
            heap,
            system_words,
            functions: Vec::new(),
        };

        let (symbols_addr, symbols_data) = module.heap.alloc_empty_block(1024)?;
        SymbolTable::new(symbols_data).init()?;

        module
            .heap
            .put(0, [0xdeadbeef, symbols_addr, system_words])?;
        core_package(&mut module)?;
        Some(module)
    }

    pub fn add_native_fn(
        &mut self,
        name: &str,
        func: crate::core::NativeFn<T, B>,
        arity: u32,
    ) -> Option<()> {
        let index = self.functions.len() as u32;
        self.functions.push(FuncDesc { func, arity });
        let symbol = inline_string(name)?;
        let id = self.get_symbols_mut()?.get_or_insert(symbol)?;
        let mut words = self
            .heap
            .get_block_mut(self.system_words)
            .map(Context::new)?;
        words.put(id, [Value::TAG_NATIVE_FN, index])
    }

    pub fn eval(&mut self, block: Offset) -> Option<[Word; 2]> {
        let mut exec = crate::core::Exec::new(self)?;
        exec.call(block)?;
        exec.eval()
    }

    pub fn parse(&mut self, input: &str) -> Result<Offset, CoreError> {
        crate::parse::parse(self, input)
    }

    /// Get a mutable reference to the heap
    ///
    /// This is primarily used by the builders. Users should generally use the
    /// context_builder() and block_builder() methods instead.
    pub fn get_heap_mut(&mut self) -> &mut Heap<T> {
        &mut self.heap
    }

    pub fn get_symbols_mut(&mut self) -> Option<SymbolTable<&mut [Word]>> {
        let addr = self.heap.get_mut::<1>(Self::SYMBOLS).map(|[addr]| *addr)?;
        self.heap.get_block_mut(addr).map(SymbolTable::new)
    }

    // Factory methods for builders

    /// Create a context builder using this module's heap
    pub fn context_builder(&mut self) -> crate::builders::ContextBuilder<T> {
        crate::builders::ContextBuilder::new(&mut self.heap)
    }

    /// Create a context builder with explicit capacity
    pub fn context_builder_with_capacity(
        &mut self,
        capacity: usize,
    ) -> crate::builders::ContextBuilder<T> {
        crate::builders::ContextBuilder::with_capacity(&mut self.heap, capacity as Offset)
    }

    /// Create a block builder using this module's heap
    pub fn block_builder(&mut self) -> crate::builders::BlockBuilder<T> {
        crate::builders::BlockBuilder::new(&mut self.heap)
    }
}

impl<T, B> Module<T, B> {
    /// Get the system words context - available for all module types
    pub fn system_words(&self) -> Offset {
        self.system_words
    }
}

impl<T, B> Module<T, B>
where
    T: AsRef<[Word]>,
    B: BlobStore,
{
    /// Get an array of values from the heap
    pub fn get_array<const N: usize>(&self, addr: Offset) -> Option<[Word; N]> {
        self.heap.get(addr)
    }

    /// Get a slice of values from a block
    pub fn get_block<const N: usize>(&self, block: Offset, offset: Offset) -> Option<[Word; N]> {
        let offset = offset as usize;
        self.heap
            .get_block(block)
            .and_then(|block| block.get(offset..offset + N))
            .and_then(|value| value.try_into().ok())
    }

    /// Get a read-only reference to the heap
    pub fn get_heap(&self) -> &Heap<T> {
        &self.heap
    }

    /// Get a function descriptor by index
    pub fn get_func(&self, index: u32) -> Option<&FuncDesc<T, B>> {
        self.functions.get(index as usize)
    }
    
    /// Get a blob from the blob store
    pub fn get_blob(&self, hash: &Hash) -> Result<&[u8], CoreError> {
        self.store.get(hash)
    }
}

impl<T, B> Module<T, B>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
    B: BlobStore,
{
    /// Store a blob in the blob store and return its hash
    pub fn store_blob(&mut self, data: &[u8]) -> Result<Hash, CoreError> {
        self.store.put(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::MemoryBlobStore;
    use crate::core::Value;

    // Create a module to use for testing
    fn setup_module() -> Module<Box<[Word]>, MemoryBlobStore> {
        let memory = vec![0; 0x10000].into_boxed_slice();
        let blob_store = MemoryBlobStore::new();
        Module::init(memory, blob_store).expect("Failed to initialize module")
    }

    #[test]
    fn test_module_builders() {
        // Create a module
        let mut module = setup_module();

        // Create a context with the module's context builder
        let context_value = module
            .context_builder()
            .with("age", 42)
            .with("name", "Test User")
            .with("active", true)
            .with("none", Value::None)
            .build()
            .expect("Failed to build context");

        // Extract the context offset
        let ctx_offset = match context_value {
            Value::Context(offset) => offset,
            _ => panic!("Expected Context value"),
        };

        // Create a block with the module's block builder
        let block_value = module
            .block_builder()
            .with_int(42)
            .with_string("Hello")
            .with_bool(true)
            .with_context(ctx_offset) // Reference the context we created
            .build()
            .expect("Failed to build block");

        // Verify block creation
        let block_offset = match block_value {
            Value::Block(offset) => offset,
            _ => panic!("Expected Block value"),
        };

        // Verify that the block exists and has the correct length
        let block_data = module
            .get_heap()
            .get_block(block_offset)
            .expect("Failed to get block");

        // The block should have 4 values, each taking 2 words (tag, data)
        assert_eq!(block_data.len(), 8);
    }
    
    #[test]
    fn test_blob_store() {
        // Create a module
        let mut module = setup_module();
        
        // Test data
        let test_data = b"This is test data for BlobStore";
        
        // Store data in blob store
        let hash = module.store_blob(test_data).expect("Failed to store blob");
        
        // Retrieve data
        let retrieved_data = module.get_blob(&hash).expect("Failed to retrieve blob");
        
        // Verify data
        assert_eq!(retrieved_data, test_data);
    }
}
