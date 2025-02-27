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

# Run Rebel Shell (interactive environment)
cargo run --package rebel-sh
```

## Rebel Overview
- **Rebel**: A programming language and interpreter developed as part of the Huly project
- **Rebel Shell**: Interactive environment that uses Rebel for scripting and command execution
- **Use Cases**: Shell scripting, data processing, and system automation

### Built-in Modules
The Rebel interpreter includes several built-in modules:

#### Filesystem Module (fs)
The `fs` module provides filesystem operations:

- **ls**: List files in the current directory
- *(Future additions: cat, mkdir, rm, etc.)*

Usage in Rebel Shell:
```
ls  // Lists files in current directory
```

To use the fs module in your Rebel applications:
```rust
// Register filesystem commands
fs::register_fs_commands(&mut module)?;
```

### Implementing Custom Commands
Rebel can be extended with custom commands implemented as native functions. Here's the basic pattern:

```rust
// 1. Define the command function
fn my_command<T>(exec: &mut Exec<T>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Implement command functionality
    // Can use exec.pop() to get arguments
    // Use exec.push() to return results
    exec.push([Value::TAG_INLINE_STRING, offset])
}

// 2. Register the command in a setup function
fn register_commands<T>(module: &mut Module<T>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Register with name, function pointer, and arity (number of arguments)
    module.add_native_fn("my_command", my_command, 0)?;
    Some(())
}

// 3. Call the registration function in your application
fn main() -> Result<()> {
    let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
    register_commands(&mut module)?;
    // ... rest of setup
}
```

When creating reusable command modules:
1. Place related commands in a dedicated module (like `fs.rs`)
2. Implement a registration function that adds all commands to a module
3. Document the arity (number of arguments) for each command
4. Consider string size limitations when returning results

## Code Style Guidelines
- **License Header**: Start files with license header: `// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT`
- **Imports**: Group standard lib, external deps, then internal imports with a blank line between groups
- **Error Handling**: Use `thiserror` for defining error types, implement `Error` trait for custom errors
- **Naming**: Use snake_case for functions/variables, CamelCase for types, SCREAMING_CASE for constants
- **Tests**: Write unit tests in a `tests` module at the bottom of each file
- **Documentation**: Document public APIs with clear descriptions and examples
- **Types**: Use Rust's strong type system; avoid raw pointers when possible
- **Error Propagation**: Use `?` operator for error propagation, not `.unwrap()` or `.expect()`

## RebelDB VM Architecture

### BlobStore
The RebelDB VM uses a BlobStore for storing immutable blobs of data that can be accessed via content-addressable hashes:

```rust
// BlobStore trait defines the interface for blob storage
pub trait BlobStore {
    // Get a blob by its hash
    fn get(&self, hash: &Hash) -> Result<&[u8], CoreError>;
    
    // Store a blob and return its hash
    fn put(&mut self, data: &[u8]) -> Result<Hash, CoreError>;
}

// In-memory implementation for testing and development
let blob_store = MemoryBlobStore::new();

// Access blobs from a module
let hash = module.store_blob(data)?;
let retrieved_data = module.get_blob(&hash)?;
```

### Module as the Main Entry Point
The recommended way to interact with the RebelDB VM is through the `Module` type, which provides factory methods for creating builders:

```rust
use rebel::{Module, Value, MemoryBlobStore};

// Create a module with memory and blob storage
let memory = vec![0; 0x10000].into_boxed_slice();
let blob_store = MemoryBlobStore::new();
let mut module = Module::init(memory, blob_store)?;

// Create context and block builders from the module
let context_builder = module.context_builder();
let block_builder = module.block_builder();
```

### Context Creation
Create contexts using the Module's context builder methods:

```rust
use rebel::{Module, Value, BlockOffset, WordRef};

// Get a context builder from the module (no direct heap access)
let ctx_value = module.context_builder()
    .with("age", 42)                        // i32 -> Int
    .with("name", "Test User")              // &str -> String
    .with("active", true)                   // bool -> Bool
    .with("none", Value::None)              // Direct value
    .build()?;                              // Returns Value::Context

// You can specify an explicit capacity if needed
let ctx_value = module.context_builder_with_capacity(20)
    .with("age", 42)
    .build()?;

// The build() method returns a Value variant (Value::Context)
match ctx_value {
    Value::Context(offset) => println!("Context created at offset {}", offset),
    _ => panic!("Expected Context value"),
}

// For references to other VM objects, use wrapper types:
let parent_ctx_value = module.context_builder().with("x", 100).build()?;

let ctx_value = module.context_builder()
    // Block references need BlockOffset wrapper
    .with("code", BlockOffset(block_offset))
    // Context references can use Value::Context directly
    .with("parent", parent_ctx_value)
    // Word references use WordRef wrapper
    .with("symbol", WordRef("some_word".to_string()))
    .build()?;
```

### Block Creation
Create blocks using the Module's block builder method:

```rust
use rebel::{Module, Value, BlockOffset, WordRef};

// Get a block builder from the module (no direct heap access)
let block_value = module.block_builder()
    .with_int(42)
    .with_string("Hello") 
    .with_bool(true)
    .with_none()
    .build()?;                        // Returns Value::Block

// The build() method returns a Value variant (Value::Block)
match block_value {
    Value::Block(offset) => println!("Block created at offset {}", offset),
    _ => panic!("Expected Block value"),
}

// Create nested blocks
let inner_block_value = module.block_builder()
    .with_int(10)
    .with_string("Inner")
    .build()?;
    
// Outer block that references the inner block
let outer_block_value = module.block_builder()
    .with_int(42)
    .with(inner_block_value)         // Pass Value directly 
    .with(ctx_value)                 // Pass Value directly
    .with_word("print")              // Word reference
    .build()?;
```

### IntoValue Trait
Both builders use the generic `with<T>()` method which accepts anything implementing `IntoValue`:
- i32 → Int VM value
- &str and String → String VM value
- bool → Bool VM value
- Offset → Context VM value
- BlockOffset(Offset) → Block VM value
- WordRef(String) → Word VM value
- Value → Direct value

## Commit Rules

- We're using git
- Always sign-off commits
- Only commit code that compiles and all tests succeed
- Make sure that clippy is happy before committing

## Preserve Knowledge

- Always add important notes and keep knowledge up to date in this document (CLAUDE.md). Feel free to fix it and add new sections as needed.
