// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

mod boot;
pub mod blob;
pub mod builders;
pub mod core;
pub mod fs;
mod hash;
pub mod mem;
pub mod module;
mod parse;

// Re-export important types
pub use blob::MemoryBlobStore;
pub use builders::{BlockBuilder, ContextBuilder};
pub use core::{inline_string, BlockOffset, CoreError, Exec, IntoValue, Value, WordRef};
pub use mem::{Context, Heap, Offset, Stack, Symbol, SymbolTable, Word};
pub use module::{BlobStore, Hash, Module};
