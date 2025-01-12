// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// runtime.rs:

/// Common WebAssembly value types
#[derive(Debug, Clone)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

/// Common error types across all runtimes
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    #[error("Failed to instantiate module: {0}")]
    Instantiation(String),
    #[error("Runtime error: {0}")]
    Runtime(String),
    #[error("Memory error: {0}")]
    Memory(String),
    #[error("Function not found: {0}")]
    FunctionNotFound(String),
}

pub type Result<T> = std::result::Result<T, WasmError>;

/// Abstract memory interface
pub trait WasmMemory {
    fn size(&self) -> usize;
    fn grow(&mut self, pages: u32) -> Result<()>;
    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<()>;
    fn write(&mut self, offset: usize, data: &[u8]) -> Result<()>;
}

/// Abstract instance interface
pub trait WasmInstance {
    fn get_memory(&mut self, name: &str) -> Result<Box<dyn WasmMemory + '_>>;
    fn call_function(&mut self, name: &str, params: &[WasmValue]) -> Result<Vec<WasmValue>>;
}

/// The main runtime trait that all platforms will implement
pub trait WasmRuntime {
    fn instantiate_module(&mut self, wasm_bytes: &[u8]) -> Result<Box<dyn WasmInstance>>;

    // Optional method for runtime-specific configurations
    fn with_config(config: RuntimeConfig) -> Result<Self>
    where
        Self: Sized;
}

/// Configuration options for runtime initialization
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    pub memory_pages: Option<u32>,
    pub enable_threads: bool,
    pub enable_simd: bool,
}
