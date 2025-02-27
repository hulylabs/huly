// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::boot::core_package;
use crate::core::{CoreError, Value};
use crate::mem::{Context, Heap, Offset, SymbolTable, Word, Symbol};

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
    ) -> Option<()> 
    where 
        T: AsMut<[Word]> + AsRef<[Word]>
    {
        let index = self.functions.len() as u32;
        self.functions.push(FuncDesc { func, arity });
        
        // Get the symbol ID for the name
        let symbol_id = self.get_or_create_symbol(name).ok()?;
        
        // Get the context and store the function
        let mut words = self
            .heap
            .get_block_mut(self.system_words)
            .map(Context::new)?;
        
        words.put(symbol_id, [Value::TAG_NATIVE_FN, index])
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
    
    /// Process a string value for a builder
    /// 
    /// This is a helper method for builders to properly create string values
    /// in the VM, using either inline strings or blob storage as appropriate.
    /// 
    /// Returns the underlying [tag, data] pair ready to be used in the VM.
    pub fn process_string_for_builder(&mut self, s: &str) -> Result<[Word; 2], CoreError> {
        // Create the string
        let string_value = self.create_string(s)?;
        
        // Extract the [tag, data] pair from the heap
        if let Value::String(offset) = string_value {
            self.heap.get(offset).ok_or(CoreError::BoundsCheckFailed)
        } else {
            // This should never happen as create_string always returns Value::String
            Err(CoreError::InternalError)
        }
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
    
    /// Get or create a symbol ID for a word
    fn get_or_create_symbol(&mut self, word: &str) -> Result<Symbol, CoreError> 
    where 
        T: AsMut<[Word]> + AsRef<[Word]>
    {
        use crate::core::inline_string;
        
        // Convert to inline string
        let word_inline = inline_string(word).ok_or(CoreError::StringTooLong)?;
        
        // Get or insert into symbol table - special handling since this is the module
        let symbols_addr = self.heap.get::<1>(Self::SYMBOLS).ok_or(CoreError::InternalError)?;
        let symbols_addr = symbols_addr[0];
        let symbols_block = self.heap.get_block_mut(symbols_addr).ok_or(CoreError::InternalError)?;
        let mut sym_tbl = SymbolTable::new(symbols_block);
        
        sym_tbl.get_or_insert(word_inline).ok_or(CoreError::SymbolTableFull)
    }
    
    /// Create a word value from a string
    pub fn create_word(&mut self, word: &str) -> Result<Value, CoreError> 
    where 
        T: AsMut<[Word]> + AsRef<[Word]>
    {
        let symbol = self.get_or_create_symbol(word)?;
        Ok(Value::Word(symbol))
    }
    
    /// Create a set-word value from a string
    pub fn create_set_word(&mut self, word: &str) -> Result<Value, CoreError> 
    where 
        T: AsMut<[Word]> + AsRef<[Word]>
    {
        let symbol = self.get_or_create_symbol(word)?;
        Ok(Value::SetWord(symbol))
    }
    
    // The create_word_from_ref method has been removed as it's no longer needed
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
    
    /// Create a string value in the heap, either inline or blob-based depending on length
    /// 
    /// This function handles the logic of choosing between inline strings (≤31 bytes)
    /// and blob-based strings (>31 bytes).
    /// 
    /// Returns a Value::String variant that points to a heap allocation containing the
    /// [tag, data] pair for the string representation.
    pub fn create_string(&mut self, s: &str) -> Result<crate::core::Value, CoreError> {
        use crate::core::{Value, inline_string};
        
        // First allocate the string data in the appropriate format
        let vm_value = if let Some(inline) = inline_string(s) {
            // Create inline string representation in heap
            let offset = self.heap.alloc(inline).ok_or(CoreError::OutOfMemory)?;
            [Value::TAG_INLINE_STRING, offset]
        } else {
            // For longer strings, store in blob store and reference the hash in heap
            let hash = self.store_blob(s.as_bytes())?;
            
            // Convert hash bytes to Words and store in a block
            let hash_words: Vec<Word> = hash.iter().map(|&b| b as Word).collect();
            let hash_offset = self.heap.alloc_block(&hash_words).ok_or(CoreError::OutOfMemory)?;
            
            [Value::TAG_STRING, hash_offset]
        };
        
        // Now allocate the [tag, data] pair in the heap
        let offset = self.heap.alloc(vm_value).ok_or(CoreError::OutOfMemory)?;
        
        // Return a Value::String that points to this heap location
        Ok(Value::String(offset))
    }
    
    /// Extract a string from a Value::String
    /// 
    /// This function handles extracting the string content from a Value::String.
    pub fn extract_string_from_value(&self, string_value: &crate::core::Value) -> Result<String, CoreError> {
        use crate::core::Value;
        
        if let Value::String(offset) = string_value {
            self.extract_string_from_offset(*offset)
        } else {
            Err(CoreError::InternalError)
        }
    }
    
    /// Extract a string from VM representation given an offset (where the tag/data pair is stored)
    /// 
    /// This function examines the tag at the offset to determine if it's an inline or blob-based string,
    /// then extracts the string appropriately.
    pub fn extract_string_from_offset(&self, offset: Offset) -> Result<String, CoreError> {
        // Get the tag and data from the offset
        let [tag, data] = self.get_array::<2>(offset).ok_or(CoreError::BoundsCheckFailed)?;
        self.extract_string(tag, data)
    }
    
    /// Extract a string from VM representation, whether inline or blob-based
    /// 
    /// This function handles both inline strings and blob-based strings based on the tag.
    /// It requires the tag/data pair that was created by create_string.
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

        // Create a string value first
        let name_value = module.create_string("Test User")
            .expect("Failed to create string");
            
        // Create a context with the module's context builder
        let context_value = module
            .context_builder()
            .with("age", 42)
            .with("name", name_value)  // Use the string value we created
            .with("active", true)
            .with("none", Value::None)
            .build()
            .expect("Failed to build context");

        // Extract the context offset
        let ctx_offset = match context_value {
            Value::Context(offset) => offset,
            _ => panic!("Expected Context value"),
        };

        // Create another string for the block
        let hello_value = module.create_string("Hello")
            .expect("Failed to create hello string");
        
        // Create a block with the module's block builder
        let block_value = module
            .block_builder()
            .with_int(42)
            .with(hello_value)        // Use the string value we created
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
        
        // 1. Test short string (31 bytes or less) - should use inline representation
        let short_string = "Hello, world!";
        
        // Create the short string in the heap
        let string_value = module.create_string(short_string)
            .expect("Failed to create short string");
        
        // Verify it's a String value
        if let Value::String(offset) = string_value {
            // Verify the offset is valid
            assert!(offset > 0);
        } else {
            panic!("Expected String value");
        }
        
        // Extract the string content
        let extracted_via_value = module.extract_string_from_value(&string_value)
            .expect("Failed to extract string from value");
        assert_eq!(extracted_via_value, short_string);
        
        // Get the underlying representation
        if let Value::String(offset) = string_value {
            let [tag, data] = module.heap.get(offset).expect("Failed to get string data");
            assert_eq!(tag, Value::TAG_INLINE_STRING);
        
            // Test extract_string directly with tag/data
            let extracted_short = module.extract_string(tag, data)
                .expect("Failed to extract short string");
            assert_eq!(extracted_short, short_string);
        
            // Test extract_string_from_offset with the offset
            let extracted_short_from_offset = module.extract_string_from_offset(offset)
                .expect("Failed to extract short string from offset");
            assert_eq!(extracted_short_from_offset, short_string);
        }
        
        // 2. Test long string (more than 31 bytes) - should use blob store
        let long_string = "This is a very long string that should definitely be stored in the blob store because it exceeds the inline string limit of 31 bytes by quite a bit.";
        
        // Create the long string
        let string_value_long = module.create_string(long_string)
            .expect("Failed to create long string");
        
        // Verify it's a String value
        if let Value::String(offset) = string_value_long {
            // Verify the offset is valid
            assert!(offset > 0);
        } else {
            panic!("Expected String value");
        }
        
        // Extract the string content
        let extracted_long_via_value = module.extract_string_from_value(&string_value_long)
            .expect("Failed to extract string from value");
        assert_eq!(extracted_long_via_value, long_string);
        
        // Get the underlying representation
        if let Value::String(offset) = string_value_long {
            let [tag, data] = module.heap.get(offset).expect("Failed to get string data");
            assert_eq!(tag, Value::TAG_STRING);
        
            // Test extract_string directly with tag/data
            let extracted_long = module.extract_string(tag, data)
                .expect("Failed to extract long string");
            assert_eq!(extracted_long, long_string);
        
            // Test extract_string_from_offset with the offset
            let extracted_long_from_offset = module.extract_string_from_offset(offset)
                .expect("Failed to extract long string from offset");
            assert_eq!(extracted_long_from_offset, long_string);
        }
        
        // 3. Test exact boundary case (30 bytes)
        let boundary_string = "This string is exactly 30 byte";
        assert_eq!(boundary_string.len(), 30);
        
        let string_value_boundary = module.create_string(boundary_string)
            .expect("Failed to create boundary string");
        
        if let Value::String(offset) = string_value_boundary {
            let [tag, data] = module.heap.get(offset).expect("Failed to get boundary string data");
            assert_eq!(tag, Value::TAG_INLINE_STRING);
            
            let extracted_boundary = module.extract_string(tag, data)
                .expect("Failed to extract boundary string");
            assert_eq!(extracted_boundary, boundary_string);
        }
        
        // 4. Test just over boundary case (31+ bytes)
        let over_boundary_string = "This string is over 31 bytes long!!";
        assert_eq!(over_boundary_string.len(), 35);
        
        let string_value_over = module.create_string(over_boundary_string)
            .expect("Failed to create over boundary string");
        
        if let Value::String(offset) = string_value_over {
            let [tag, data] = module.heap.get(offset).expect("Failed to get over boundary string data");
            assert_eq!(tag, Value::TAG_STRING);
            
            let extracted_over = module.extract_string(tag, data)
                .expect("Failed to extract over boundary string");
            assert_eq!(extracted_over, over_boundary_string);
        }
        
        // 5. Test Unicode string (with multi-byte characters)
        let unicode_string = "Hello, 世界! 👋";
        
        let string_value_unicode = module.create_string(unicode_string)
            .expect("Failed to create Unicode string");
        
        if let Value::String(offset) = string_value_unicode {
            let [tag, data] = module.heap.get(offset).expect("Failed to get Unicode string data");
            
            let extracted_unicode = module.extract_string(tag, data)
                .expect("Failed to extract Unicode string");
            assert_eq!(extracted_unicode, unicode_string);
        }
        
        // 6. Test empty string
        let empty_string = "";
        
        let string_value_empty = module.create_string(empty_string)
            .expect("Failed to create empty string");
        
        if let Value::String(offset) = string_value_empty {
            let [tag, data] = module.heap.get(offset).expect("Failed to get empty string data");
            assert_eq!(tag, Value::TAG_INLINE_STRING);
            
            let extracted_empty = module.extract_string(tag, data)
                .expect("Failed to extract empty string");
            assert_eq!(extracted_empty, empty_string);
        }
    }
}
