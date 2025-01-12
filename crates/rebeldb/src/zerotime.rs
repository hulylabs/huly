//

use crate::runtime::{
    Result, RuntimeConfig, WasmError, WasmInstance, WasmMemory, WasmRuntime, WasmValue,
};

// Our own memory implementation that matches WebAssembly memory model
pub struct ZeroMemory {
    data: Vec<u8>,
    max_pages: Option<u32>,
}

pub struct ZeroMemoryRef<'a> {
    data: &'a mut Vec<u8>,
    max_pages: Option<u32>,
}

impl ZeroMemory {
    const PAGE_SIZE: usize = 65536;

    fn new(initial_pages: u32, max_pages: Option<u32>) -> Self {
        let size = initial_pages as usize * Self::PAGE_SIZE;
        Self {
            data: vec![0; size],
            max_pages,
        }
    }
}

pub struct ZeroInstance {
    memory: ZeroMemory,
}

pub struct ZeroRuntime {
    // For now empty, might need configuration later
}

impl<'a> WasmMemory for ZeroMemoryRef<'a> {
    fn size(&self) -> usize {
        self.data.len()
    }

    fn grow(&mut self, additional_pages: u32) -> Result<()> {
        let current_pages = self.data.len() / ZeroMemory::PAGE_SIZE;
        let new_pages = current_pages + additional_pages as usize;

        if let Some(max) = self.max_pages {
            if new_pages > max as usize {
                return Err(WasmError::Memory("Exceeded maximum memory pages".into()));
            }
        }

        let additional_bytes = additional_pages as usize * ZeroMemory::PAGE_SIZE;
        self.data
            .extend(std::iter::repeat(0).take(additional_bytes));
        Ok(())
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
        if offset + buf.len() > self.data.len() {
            return Err(WasmError::Memory("Read outside memory bounds".into()));
        }
        buf.copy_from_slice(&self.data[offset..offset + buf.len()]);
        Ok(())
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        if offset + data.len() > self.data.len() {
            return Err(WasmError::Memory("Write outside memory bounds".into()));
        }
        self.data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
}

impl WasmInstance for ZeroInstance {
    fn get_memory(&mut self, _name: &str) -> Result<Box<dyn WasmMemory + '_>> {
        Ok(Box::new(ZeroMemoryRef {
            data: &mut self.memory.data,
            max_pages: self.memory.max_pages,
        }))
    }

    fn call_function(&mut self, name: &str, _params: &[WasmValue]) -> Result<Vec<WasmValue>> {
        Err(WasmError::Runtime(format!(
            "Zerotime cannot execute functions (attempted to call {})",
            name
        )))
    }
}

impl WasmRuntime for ZeroRuntime {
    fn instantiate_module(&mut self, _wasm_bytes: &[u8]) -> Result<Box<dyn WasmInstance>> {
        // For now just create an instance with default memory
        // Later we might want to parse the wasm binary to get memory specifications
        Ok(Box::new(ZeroInstance {
            memory: ZeroMemory::new(1, None), // Start with 1 page
        }))
    }

    fn with_config(_config: RuntimeConfig) -> Result<Self> {
        Ok(Self {})
    }
}
