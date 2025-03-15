# Path Access Issue Report

## Issue Description

The following code fails with an "out of bounds" error:
```
f: func [a] [print a/field] f context [field: 5]
```

This happens when trying to access a field from a context that was passed as an argument to a function.

## Reproduction

1. Create a context with a field: `ctx: context [field: 5]`
2. Define a function that tries to access a field from its argument: `f: func [a] [a/field]`
3. Call the function with the context: `f ctx`
4. Result: "out of bounds" error

## Root Cause Analysis

Through testing and code analysis, we've identified the root cause of the issue:

### How Function Arguments Work

1. In the `func` implementation in `boot.rs`, function parameters are stored as `TAG_STACK_VALUE` with an offset
2. When a function is called, arguments are pushed onto the stack
3. Inside the function, parameters are accessed via stack offsets

### How Word Resolution Works

1. In the `resolve` method in `core.rs`, there are two cases for resolving values:
   - `TAG_WORD`: For resolving a single word
   - `TAG_PATH`: For resolving a path like `a/field`

2. In the `TAG_WORD` case, when a word resolves to `TAG_STACK_VALUE`:
   ```rust
   if result[0] == VmValue::TAG_STACK_VALUE {
       let [base] = self.base.peek::<1>().ok_or(MemoryError::StackUnderflow)?;
       self.stack.get(base + result[1])
   }
   ```
   It correctly gets the actual value from the stack using the base offset and the parameter offset.

3. But in the `TAG_PATH` case, it doesn't handle `TAG_STACK_VALUE` specially:
   ```rust
   [VmValue::TAG_WORD, symbol] => {
       result = self.find_word(symbol)?;
       self.env.push([result[1]])?;
   }
   ```
   It just pushes `result[1]` (the offset) onto the environment stack, not the actual value.

### The Specific Issue

When we have `a/field` inside a function:

1. The first segment 'a' is a word that resolves to `TAG_STACK_VALUE` with an offset
2. In the `TAG_PATH` case, it doesn't handle `TAG_STACK_VALUE` specially:
   - It calls `find_word` to resolve the word 'a'
   - `find_word` returns `[TAG_STACK_VALUE, offset]`
   - It pushes `offset` onto the environment stack with `self.env.push([result[1]])?`
   - When it tries to access 'field', it's trying to use this offset as a context address
   - This causes an "out of bounds" error because the offset is not a valid context address

## The Problematic Code

The issue is in the `resolve` method in `core.rs`:

```rust
fn resolve(&mut self, value: MemValue) -> Result<MemValue, MemoryError> {
    match value[0] {
        VmValue::TAG_WORD => self.find_word(value[1]).and_then(|result| {
            if result[0] == VmValue::TAG_STACK_VALUE {
                let [base] = self.base.peek::<1>().ok_or(MemoryError::StackUnderflow)?;
                self.stack.get(base + result[1])
            } else {
                Ok(result)
            }
        }),
        VmValue::TAG_PATH => {
            let mut offset = 0;
            let block = value[1];
            let mut result = [VmValue::TAG_NONE, 0];
            while let Ok(word_value) = self.get_block(block, offset) {
                match word_value {
                    [VmValue::TAG_WORD, symbol] => {
                        result = self.find_word(symbol)?;
                        self.env.push([result[1]])?;
                    }
                    _ => unimplemented!(),
                }
                offset += 2;
            }
            let env_len = self.env.len()?;
            self.env.set_len(env_len - offset / 2)?;
            Ok(result)
        }
        _ => Ok(value),
    }
}
```

The key issue is that the `TAG_PATH` case doesn't handle `TAG_STACK_VALUE` specially like the `TAG_WORD` case does. It should get the actual value from the stack when it encounters `TAG_STACK_VALUE`, just like in the `TAG_WORD` case.

## Potential Fixes

### Approach 1: Handle TAG_STACK_VALUE Explicitly

One approach is to modify the `resolve` method to handle `TAG_STACK_VALUE` explicitly in the `TAG_PATH` case:

```rust
fn resolve(&mut self, value: MemValue) -> Result<MemValue, MemoryError> {
    match value[0] {
        VmValue::TAG_WORD => self.find_word(value[1]).and_then(|result| {
            if result[0] == VmValue::TAG_STACK_VALUE {
                let [base] = self.base.peek::<1>().ok_or(MemoryError::StackUnderflow)?;
                self.stack.get(base + result[1])
            } else {
                Ok(result)
            }
        }),
        VmValue::TAG_PATH => {
            let mut offset = 0;
            let block = value[1];
            let mut result = [VmValue::TAG_NONE, 0];
            while let Ok(word_value) = self.get_block(block, offset) {
                match word_value {
                    [VmValue::TAG_WORD, symbol] => {
                        result = self.find_word(symbol)?;
                        
                        // Handle TAG_STACK_VALUE specially
                        if result[0] == VmValue::TAG_STACK_VALUE {
                            let [base] = self.base.peek::<1>().ok_or(MemoryError::StackUnderflow)?;
                            result = self.stack.get(base + result[1])?;
                        }
                        
                        // Now push the actual value onto the environment stack
                        self.env.push([result[1]])?;
                    }
                    _ => unimplemented!(),
                }
                offset += 2;
            }
            let env_len = self.env.len()?;
            self.env.set_len(env_len - offset / 2)?;
            Ok(result)
        }
        _ => Ok(value),
    }
}
```

### Approach 2: Recursive Resolution

A more elegant approach would be to recursively call `resolve` to handle any indirection:

```rust
fn resolve(&mut self, value: MemValue) -> Result<MemValue, MemoryError> {
    match value[0] {
        VmValue::TAG_WORD => self.find_word(value[1]).and_then(|result| {
            if result[0] == VmValue::TAG_STACK_VALUE {
                let [base] = self.base.peek::<1>().ok_or(MemoryError::StackUnderflow)?;
                self.stack.get(base + result[1])
            } else {
                Ok(result)
            }
        }),
        VmValue::TAG_PATH => {
            let mut offset = 0;
            let block = value[1];
            let mut result = [VmValue::TAG_NONE, 0];
            while let Ok(word_value) = self.get_block(block, offset) {
                match word_value {
                    [VmValue::TAG_WORD, symbol] => {
                        // First, find the word
                        let word_result = self.find_word(symbol)?;
                        
                        // Then recursively resolve it if it's a TAG_STACK_VALUE
                        if word_result[0] == VmValue::TAG_STACK_VALUE {
                            // Create a temporary TAG_WORD value to resolve
                            let temp_word = [VmValue::TAG_WORD, symbol];
                            result = self.resolve(temp_word)?;
                        } else {
                            result = word_result;
                        }
                        
                        // Now push the actual value onto the environment stack
                        self.env.push([result[1]])?;
                    }
                    _ => unimplemented!(),
                }
                offset += 2;
            }
            let env_len = self.env.len()?;
            self.env.set_len(env_len - offset / 2)?;
            Ok(result)
        }
        _ => Ok(value),
    }
}
```

This recursive approach has several advantages:
1. It reuses the existing resolution logic rather than duplicating it
2. It would handle not just `TAG_STACK_VALUE` but any other indirection that might be added in the future
3. If the resolution logic for `TAG_WORD` changes, the `TAG_PATH` case would automatically benefit

However, care must be taken to avoid infinite recursion in case of circular references.

## Conclusion

The issue is that the `TAG_PATH` case in the `resolve` method doesn't handle `TAG_STACK_VALUE` specially like the `TAG_WORD` case does. When a function parameter is accessed via a path (like `a/field`), it pushes the raw offset onto the environment stack instead of getting the actual value from the stack.

Two potential fixes are:
1. Explicitly handle `TAG_STACK_VALUE` in the `TAG_PATH` case, similar to how it's handled in the `TAG_WORD` case
2. Use a recursive approach to leverage the existing resolution logic, which would be more elegant and maintainable

The recursive approach is generally preferred as it's more general and maintainable, but care must be taken to avoid infinite recursion.
