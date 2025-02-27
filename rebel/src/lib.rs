// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

mod boot;
pub mod core;
mod hash;
pub mod mem;
mod parse;
pub mod context_builder;

// Re-export important types
pub use core::{Module, Exec, Value, CoreError, inline_string};
pub use mem::{Heap, Context, Stack, SymbolTable, Word, Offset, Symbol};
pub use context_builder::{ContextBuilder, ContextValue, IntoContextValue, BlockOffset, WordRef};
