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
- **Tests**: 
  - Write unit tests in a `tests` module at the bottom of each file
  - Tests may return `()` directly or `Result<(), Error>` as needed
  - Prefer using `.expect()` with meaningful messages over `.unwrap()`
  - Avoid using `println!` in tests, use assertions to validate behavior
  - Follow the arrange-act-assert pattern for test structure
- **Documentation**: Document public APIs with clear descriptions and examples
- **Types**: Use Rust's strong type system; avoid raw pointers when possible
- **Error Propagation**: Use `?` operator for error propagation, not `.unwrap()` or `.expect()`
- **Unwrap Avoidance**: Avoid `unwrap()` calls; use pattern matching, `ok_or()`, or proper error handling instead
- **Panic Prevention**: Return `Option` or `Result` instead of panicking; use `.get()` instead of indexing for safe array/slice access
- **Clippy**: Run `cargo clippy` to catch common mistakes and improve code quality
- **Comments**: Avoid inline comments, write self-explanatory code, use comments for complex logic
- **Rust Docs**: Write Rust docs for public APIs, use `cargo doc --open` to generate and view docs

## Code Patterns
- **Visitor Pattern**: 
  - Used for parsing with `Collector` trait (`parse.rs`, `collector.rs`) 
  - Used for serialization with `Serializer` trait (`serialize.rs`)
  - Allows creating new formats without modifying value representation
  
- **Value Serialization**:
  - Binary serialization is available using `to_bytes` and `from_bytes` functions
  - Custom serializers can be implemented using the `Serializer` trait
  - Variable-length integer encoding used for efficient space utilization

## REBOL Language Features
RebelDB is inspired by REBOL (Relative Expression-Based Object Language), and implements many of its concepts:

- **Value Types**:
  - `Value::None`: Represents absence of a value
  - `Value::Int`: Integer values
  - `Value::String`: UTF-8 string values using SmolStr for efficiency
  - `Value::Word`/`Value::SetWord`: Symbolic references and assignments
  - `Value::Block`: Ordered collections of values (arrays/lists)
  - `Value::Context`: Key-value pairs, similar to objects or dictionaries

- **Context Implementation**:
  - Implemented as `Box<[(SmolStr, Value)]>` for memory efficiency
  - Used for object-like structures, configurations, and namespaces
  - Serialized with efficient binary encoding in the `serialize.rs` module

- **Serialization**:
  - All Value types support serialization/deserialization
  - Binary format is compact and preserves all value semantics
  - Tagged format with variable-length encoding for efficient size
  - Serialization API via Visitor pattern for extensibility
  - Strong error types with specific error variants
  - Support for streaming I/O with Reader/Writer interfaces

The binary serialization format uses:
  - Type tag byte (matching Tag constants in core.rs)
  - Variable-length integer encoding for numbers and lengths
  - UTF-8 encoding for string data
  - Recursive encoding for nested structures (Block, Context)

For detailed documentation, see:
  - [REBOL Design Principles](docs/rebol-design.md)
  - [Binary Serialization Format](docs/binary-serialization.md)

## Commits

- Make sure project compiles and tests pass before committing.
- Make sure clippy linter passes before committing.

## Knowledge Base

- Always update Claude's knowledge base with new learnings, best practices, and gotchas
