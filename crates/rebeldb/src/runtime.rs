// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// runtime.rs:

//! WebAssembly Runtime Abstraction
//!
//! This crate provides a platform-agnostic abstraction layer for WebAssembly runtime operations.
//! The goal is to provide a consistent interface for working with WebAssembly across different
//! environments and runtime implementations.
//!
//! # Design Principles
//!
//! - **Runtime Agnostic**: The abstraction should work with any WebAssembly runtime (Wasmtime, Wasmer, etc.)
//!   without being tied to specific runtime implementation details.
//!
//! - **Platform Independent**: While not all platforms may support all features, the core abstractions
//!   should be usable across native, web, and mobile environments.
//!
//! - **Memory-First**: WebAssembly memory operations are fundamental and should work consistently
//!   across all implementations, even when execution capabilities vary.
//!
//! - **Minimal Runtime Requirements**: Implementations can provide different levels of functionality,
//!   from full execution environments to minimal memory-only implementations.
//!
//! # Runtime Implementations
//!
//! ## Wasmtime Runtime
//!
//! A full-featured implementation using Wasmtime as the backend. Provides complete WebAssembly
//! execution capabilities including host functions, memory operations, and module instantiation.
//!
//! ## Zerotime Runtime
//!
//! A special "null" implementation that provides memory management without execution capabilities.
//! This implementation is useful for:
//! - Testing and development without full runtime overhead
//! - Memory preparation and manipulation separate from execution
//! - Scenarios where only memory operations are needed
//! - As a reference implementation showing minimal requirements for a runtime
//!
//! # Use Cases
//!
//! This abstraction is particularly useful for:
//!
//! - **Cross-Platform Applications**: Write WebAssembly interaction code once and run it anywhere
//! - **Testing and Development**: Use lighter implementations like zerotime for testing
//! - **Memory Management**: Handle WebAssembly memory consistently across different platforms
//! - **Runtime Switching**: Easily swap between different WebAssembly runtimes based on needs
//!
//! # Memory Model
//!
//! All implementations share the same WebAssembly memory model:
//! - Memory is organized in 64KB pages
//! - Linear memory is represented as contiguous bytes
//! - Standard operations: read, write, grow
//! - Memory can be shared between different runtime implementations
//!
//! # Example
//!
//! ```rust,no_run
//! use rebeldb::runtime::{WasmRuntime, RuntimeConfig};
//!
//! // This code will work with any runtime implementation
//! fn process_wasm<R: WasmRuntime>(runtime: &mut R, wasm_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//!     let instance = runtime.instantiate_module(wasm_bytes)?;
//!     let memory = instance.get_memory("memory")?;
//!     // ... work with memory
//!     Ok(())
//! }
//! ```

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
