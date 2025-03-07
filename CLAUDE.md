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

## Repository Rules

- We're using git
- Always sign-off commits
