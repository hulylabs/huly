// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

mod boot;
pub mod core;
mod hash;
pub mod mem;
mod parse;
pub mod builders;
pub mod module;

// Re-export important types
pub use core::{Exec, Value, CoreError, inline_string, IntoValue, BlockOffset, WordRef};
pub use mem::{Heap, Context, Stack, SymbolTable, Word, Offset, Symbol};
pub use builders::{ContextBuilder, BlockBuilder};
pub use module::Module;
