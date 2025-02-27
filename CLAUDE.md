# Huly Codebase Guide

## Build/Test Commands
```bash
# Build the entire project
cargo build

# Run tests
cargo test

# Run a specific test
cargo test test_name

# Run specific module tests
cargo test --package rebel core::tests

# Check for errors without building
cargo check

# Format code
cargo fmt

# Run clippy linter
cargo clippy
```

## Code Style Guidelines
- **License Header**: Start files with license header: `// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT`
- **Imports**: Group standard lib, external deps, then internal imports with a blank line between groups
- **Error Handling**: Use `thiserror` for defining error types, implement `Error` trait for custom errors
- **Naming**: Use snake_case for functions/variables, CamelCase for types, SCREAMING_CASE for constants
- **Tests**: Write unit tests in a `tests` module at the bottom of each file
- **Documentation**: Document public APIs with clear descriptions and examples
- **Types**: Use Rust's strong type system; avoid raw pointers when possible
- **Error Propagation**: Use `?` operator for error propagation, not `.unwrap()` or `.expect()`

## RebelDB VM Context Creation
The recommended way to create contexts in RebelDB is using the `ContextBuilder` API:

```rust
use rebel::{ContextBuilder, Value, BlockOffset, WordRef};

// Basic value types use automatic type inference
let ctx = ContextBuilder::new(heap, 10)
    .with("age", 42)                        // i32 -> Int
    .with("name", "Test User")              // &str -> String
    .with("active", true)                   // bool -> Bool
    .with("none", Value::None)              // Direct value
    .build()?;

// For references to other VM objects, use wrapper types:
let ctx = ContextBuilder::new(heap, 10)
    // Block references need BlockOffset wrapper
    .with("code", BlockOffset(block_offset))
    // Context references use Offset directly
    .with("parent", parent_ctx)
    // Word references use WordRef wrapper
    .with("symbol", WordRef("some_word".to_string()))
    .build()?;
```

The generic `with<T>()` method accepts anything implementing `IntoValue`:
- i32 → Int VM value
- &str and String → String VM value
- bool → Bool VM value
- Offset → Context VM value
- BlockOffset(Offset) → Block VM value
- WordRef(String) → Word VM value
- Value → Direct value

## Repository Rules

- We're using git
- Always sign-off commits

## Preserve Knowledge

- Always add important notes and keep knowledge up to date in this document (CLAUDE.md). Feel free to fix it and add new sections as needed.
