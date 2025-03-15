# RebelDB VM Integration Guide

This document describes how the high-level `Value` types integrate with the RebelDB virtual machine (VM) using direct memory representations.

## Overview

RebelDB uses a dual representation for data:

1. **High-level Values** (`Value` enum): Memory-safe, type-safe representations used in application code
2. **Low-level VM memory** (`Word` arrays): Compact, efficient representations used by the VM

The `vm_serialize` module provides direct, bidirectional mapping between these representations, enabling:

- Converting Values to VM memory representation
- Executing code in the VM
- Extracting Values from VM memory

## Direct VM Conversion

Unlike the string-based serialization in binary format, VM conversion directly maps Values to their corresponding memory layout in the VM:

| Value Type | VM Representation |
|------------|------------------|
| None       | `[TAG_NONE, 0]` |
| Int        | `[TAG_INT, value]` |
| String     | `[TAG_INLINE_STRING, offset]` where offset points to packed string data |
| Word       | `[TAG_WORD, symbol_id]` where symbol_id is entry in symbol table |
| SetWord    | `[TAG_SET_WORD, symbol_id]` |
| Block      | `[TAG_BLOCK, offset]` where offset points to sequence of values |
| Context    | `[TAG_CONTEXT, offset]` where offset points to context structure |

## Key Components

### ValueCollector

The `ValueCollector` converts Values directly to VM memory using the VM's memory management:

```rust
// Create a collector with a module reference
let mut collector = ValueCollector::new(&mut module);

// Convert a Value to VM memory
let block_offset = collector.to_vm(&value)?;
```

The collector implements the `Collector` trait, reusing the same visitor pattern used by the parser, enabling unified processing of values.

### ValueExtractor

The `ValueExtractor` reads VM memory and reconstructs the corresponding Value:

```rust
// Create an extractor with a module reference
let extractor = ValueExtractor::new(&module);

// Extract a Value from VM result
let value = extractor.extract(vm_result)?;
```

### High-level Functions

The module provides high-level functions for common operations:

```rust
// Execute a Value in the VM and get the raw result
let vm_result = execute(&value, &mut module)?;

// Execute a Value and convert result back to a Value
let result = eval(&value, &mut module)?;
```

## VM Execution Cycle

A complete VM execution cycle:

1. Create a VM module
```rust
let memory = vec![0; 0x10000].into_boxed_slice();
let mut module = Module::init(memory)?;
```

2. Create a Value to execute
```rust
let value = parse("add 1 2").unwrap();
```

3. Convert to VM representation
```rust
let mut collector = ValueCollector::new(&mut module);
let block_offset = collector.to_vm(&value)?;
```

4. Execute in the VM
```rust
let vm_result = module.eval(block_offset)?;
```

5. Extract result back to a Value
```rust
let extractor = ValueExtractor::new(&module);
let result = extractor.extract(vm_result)?;
```

Or simply:
```rust
let result = eval(&value, &mut module)?;
```

## Implementation Details

### Context Creation

Context creation requires special handling:

1. Create a block with field definitions (set-words and values)
2. Create a call to the `context` function with that block
3. Execute the call to transform the block into a context
4. Return the resulting context

### String Handling

Strings are stored in a packed byte format with length prefix:

```
[length, byte0|byte1|byte2|byte3, byte4|...]
```

Where:
- `length` is the string length in bytes
- Each subsequent word contains up to 4 bytes of string data

### Symbol Table

Words are stored as references to entries in the symbol table:

1. Convert the word string to inline string format
2. Insert or retrieve the symbol ID from the symbol table
3. Store `[TAG_WORD, symbol_id]` or `[TAG_SET_WORD, symbol_id]`

## Advanced Usage

### Creating Context Values

```rust
// Create a context directly
let context = Value::Context(Box::new([
    (SmolStr::new("name"), Value::String("John".into())),
    (SmolStr::new("age"), Value::Int(30)),
]));

// Convert to VM representation
let offset = collector.to_vm(&context)?;
```

### Extracting Context Fields

```rust
// Create a context
let context_value = parse("context [x: 10 y: 20]").unwrap();
let mut collector = ValueCollector::new(&mut module);
let offset = collector.to_vm(&context_value)?;

// Execute to get the context
let vm_result = module.eval(offset).unwrap();

// Extract the context
let extractor = ValueExtractor::new(&module);
let result = extractor.extract(vm_result).unwrap();
```

## Performance Considerations

The direct memory conversion is significantly more efficient than string-based serialization:

1. No intermediate string representation is created
2. Values are directly mapped to their VM memory layout
3. Memory allocation is minimized
4. Type information is preserved

## Example: Full Code Execution

```rust
// Create a Value representing a program with function definition and call
let code = Value::Block(Box::new([
    Value::SetWord("factorial".into()),
    Value::Word("func".into()),
    Value::Block(Box::new([
        Value::Word("n".into()),
    ])),
    Value::Block(Box::new([
        Value::Word("either".into()),
        Value::Word("lt".into()),
        Value::Word("n".into()),
        Value::Int(2),
        Value::Block(Box::new([
            Value::Int(1),
        ])),
        Value::Block(Box::new([
            Value::Word("multiply".into()),
            Value::Word("n".into()),
            Value::Word("factorial".into()),
            Value::Word("subtract".into()),
            Value::Word("n".into()),
            Value::Int(1),
        ])),
    ])),
    Value::Word("factorial".into()),
    Value::Int(5),
]));

// Execute in VM
let result = eval(&code, &mut module)?;
// Result is Value::Int(120) (5!)
```