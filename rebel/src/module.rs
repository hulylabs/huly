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
    pub(crate) store: B,
    pub(crate) heap: Heap<T>,
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
    
    /// Create a string value, either inline or blob-based depending on length
    /// 
    /// This function handles the logic of choosing between inline strings (≤31 bytes)
    /// and blob-based strings (>31 bytes).
    /// 
    /// Returns a Value with the appropriate string representation.
    pub fn create_string(&mut self, s: &str) -> Result<crate::core::Value, CoreError> {
        use crate::core::{Value, inline_string};
        
        // Try to create an inline string first
        if let Some(inline) = inline_string(s) {
            // Create inline string representation - we pre-allocate it for efficiency
            // but the actual storage happens in to_vm_value
            let _offset = self.heap.alloc(inline).ok_or(CoreError::OutOfMemory)?;
        } else {
            // For longer strings, store in blob store
            // Pre-store the blob for efficiency, but the actual storage reference happens in to_vm_value
            let _hash = self.store_blob(s.as_bytes())?;
        }
        
        // The string is always represented as a Value::String in the API
        // The blob handling is done internally by to_vm_value
        Ok(Value::String(s.to_string()))
    }
    
    /// Extract a string from VM representation, whether inline or blob-based
    /// 
    /// This function handles both inline strings and blob-based strings.
    pub fn extract_string(&self, tag: Word, data: Word) -> Result<String, CoreError> {
        use crate::core::Value;
        
        match tag {
            Value::TAG_INLINE_STRING => {
                // Handle inline string
                let inline_data = self.heap.get::<8>(data).ok_or(CoreError::BoundsCheckFailed)?;
                let len = inline_data[0] as usize;
                
                // Extract bytes from packed representation
                let mut bytes = Vec::with_capacity(len);
                for i in 0..len {
                    let j = i + 1; // Skip the length byte
                    let word_idx = j / 4;
                    
                    // Make sure we don't go out of bounds
                    if word_idx >= inline_data.len() {
                        break;
                    }
                    
                    let byte_idx = j % 4;
                    let byte = ((inline_data[word_idx] >> (byte_idx * 8)) & 0xFF) as u8;
                    bytes.push(byte);
                }
                
                // Convert bytes to string, removing any trailing zeros
                let bytes_without_nulls: Vec<u8> = bytes.into_iter()
                    .take_while(|&b| b != 0)
                    .collect();
                
                String::from_utf8(bytes_without_nulls).map_err(|_| CoreError::InternalError)
            },
            Value::TAG_STRING => {
                // Handle blob-based string
                let hash_data = self.heap.get_block(data).ok_or(CoreError::BoundsCheckFailed)?;
                
                // Convert block data to hash
                let mut hash = [0u8; 32];
                for (i, word) in hash_data.iter().enumerate().take(32) {
                    hash[i] = *word as u8;
                }
                
                // Get the blob data
                let blob = self.store.get(&hash)?;
                
                // Convert blob to string
                String::from_utf8(blob.to_vec()).map_err(|_| CoreError::InternalError)
            },
            _ => Err(CoreError::InternalError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::MemoryBlobStore;
    use crate::core::Value;

    // Create a module to use for testing
    fn setup_module() -> Module<Box<[Word]>, MemoryBlobStore> {
        let memory: Box<[Word]> = vec![0; 0x10000].into_boxed_slice();
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
    
    #[test]
    fn test_string_handling() {
        // Create a module
        let mut module = setup_module();
        
        // Short string that should be stored inline
        let short_string = "Hello";
        
        // Create a string Value
        let short_string_value = module.create_string(short_string).expect("Failed to create short string");
        
        // Extract the string to verify it works
        let extracted_short = {
            // First convert to VM representation in heap (not using to_vm_value since it works differently now)
            if let Value::String(s) = &short_string_value {
                let inline = inline_string(&s).unwrap();
                let offset = module.heap.alloc(inline).unwrap();
                module.extract_string(Value::TAG_INLINE_STRING, offset).expect("Failed to extract short string")
            } else {
                panic!("Expected string value");
            }
        };
        
        assert_eq!(extracted_short, short_string);
        
        // Test long string handling with blob store
        let long_string = "This is a very long string that should definitely be stored in the blob store because it exceeds the inline string limit of 31 bytes by quite a bit.";
        
        // Store directly in blob store
        let hash = module.store_blob(long_string.as_bytes()).expect("Failed to store blob");
        
        // Convert hash to words and store in heap
        let hash_words: Vec<Word> = hash.iter().map(|&b| b as Word).collect();
        let hash_offset = module.heap.alloc_block(&hash_words).unwrap();
        
        // Extract using TAG_STRING
        let extracted_long = module.extract_string(Value::TAG_STRING, hash_offset)
            .expect("Failed to extract long string");
        
        assert_eq!(extracted_long, long_string);
    }
}
