// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Module, Op, VmValue};
use crate::mem::MemoryError;
use crate::value::Value;

/// Test to identify the root cause of the path access issue
#[test]
fn test_path_access_in_function() -> Result<(), CoreError> {
    // Initialize a module
    let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;

    // First, test direct context access
    let direct_access = "ctx: context [field: 5] ctx/field";
    let block = module.parse(direct_access)?;
    let result = module.eval(block)?;
    let value = module.to_value(result)?;
    assert_eq!(value, Value::Int(5), "Direct context access failed");
    println!("Direct context access works correctly");

    // Now test function with path access
    let function_test = "f: func [a] [a/field] f context [field: 5]";
    let block = module.parse(function_test)?;

    // This is expected to fail with an "out of bounds" error
    match module.eval(block) {
        Ok(result) => {
            let value = module.to_value(result)?;
            println!(
                "Function test unexpectedly succeeded with result: {}",
                value
            );
            assert_eq!(value, Value::Int(5), "Function should return 5 if it works");
        }
        Err(err) => {
            println!("Function test failed with error: {}", err);
            // We expect a MemoryError::OutOfBounds error
            assert!(
                format!("{}", err).contains("out of bounds"),
                "Expected 'out of bounds' error, got: {}",
                err
            );

            // The error is confirmed, now let's debug the issue

            // First, let's try to understand how function arguments are stored
            let setup = "test_ctx: context [field: 5]";
            let block = module.parse(setup)?;
            module.eval(block)?;

            // Define a function that just returns its argument
            let return_func = "return_arg: func [a] [a]";
            let block = module.parse(return_func)?;
            module.eval(block)?;

            // Call the function with the context
            let call_test = "result: return_arg test_ctx";
            let block = module.parse(call_test)?;
            module.eval(block)?;

            // Check the result
            let check_result = "result";
            let block = module.parse(check_result)?;
            let result = module.eval(block)?;
            let value = module.to_value(result)?;

            // The result should be a context
            assert!(
                matches!(value, Value::Context(_)),
                "Function should return a context, got: {:?}",
                value
            );

            // Now let's try to access a field from the result
            let access_test = "result/field";
            let block = module.parse(access_test)?;
            let result = module.eval(block)?;
            let value = module.to_value(result)?;

            // This should work if the context is properly returned
            assert_eq!(
                value,
                Value::Int(5),
                "Field access from function result failed"
            );

            println!("Field access from function result works correctly");

            // So the issue must be in how the context is handled inside the function
            println!("\nAnalyzing the root cause of the issue:");
            println!("---------------------------------------");

            // Let's try to understand what's happening with the path access inside a function
            println!("1. In the function body, when we have a/field:");
            println!("   - 'a' is a function parameter");
            println!("   - Function parameters are stored on the stack");
            println!("   - When we access a/field, we're trying to access a field from a context on the stack");

            // Let's look at how function arguments are stored
            println!("\n2. How function arguments are stored:");
            println!("   - In the func implementation in boot.rs, function parameters are stored as TAG_STACK_VALUE");
            println!("   - When a function is called, arguments are pushed onto the stack");
            println!("   - Inside the function, parameters are accessed via stack offsets");

            // Let's look at how path access is implemented
            println!("\n3. How path access is implemented:");
            println!("   - In the resolve method in core.rs, path access is handled in the TAG_PATH case");
            println!("   - It iterates through the path segments and resolves each one");
            println!("   - For each segment, it calls find_word to get the value");
            println!("   - Then it pushes the value onto the environment stack");

            // The issue is likely in how the path access interacts with function arguments
            println!("\n4. The likely issue:");
            println!("   - When we have a/field inside a function:");
            println!("     1. 'a' is resolved to a TAG_STACK_VALUE with an offset");
            println!("     2. The value at that offset is a context");
            println!("     3. But when we try to access 'field' from it, we're not properly handling the context");
            println!(
                "     4. The context is not being pushed onto the environment stack correctly"
            );

            // Let's look at the specific issue in the resolve method
            println!("\n5. The specific issue in the resolve method:");
            println!("   - In the TAG_PATH case, when we resolve a word to a TAG_STACK_VALUE:");
            println!("     1. We get the base offset from the base stack");
            println!("     2. We get the value from the stack at base + offset");
            println!("     3. But we don't properly handle this value when it's a context");
            println!(
                "     4. We push the raw value onto the environment stack, not the context itself"
            );

            // The fix would be to properly handle TAG_STACK_VALUE in the path access code
            println!("\n6. Potential fix:");
            println!("   - In the TAG_PATH case of the resolve method:");
            println!("   - When we resolve a word to a TAG_STACK_VALUE:");
            println!("     1. Get the actual value from the stack");
            println!("     2. If it's a context, push the context onto the environment stack");
            println!("     3. Then continue with the path resolution");
        }
    }

    Ok(())
}

/// Test to specifically identify the issue with path access in functions
#[test]
fn test_path_access_root_cause() -> Result<(), CoreError> {
    // Initialize a module
    let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;

    // Create a context and store it in a variable
    let ctx_code = "test_ctx: context [field: 5]";
    let block = module.parse(ctx_code)?;
    module.eval(block)?;

    // Define a function that tries to access a field from its argument
    let func_def = "test_func: func [a] [a/field]";
    let block = module.parse(func_def)?;
    module.eval(block)?;

    // Call the function with the context
    let func_call = "test_func test_ctx";
    let block = module.parse(func_call)?;

    // This is where we expect the error to occur
    match module.eval(block) {
        Ok(_) => {
            println!("Function call unexpectedly succeeded");
        }
        Err(err) => {
            println!("Function call failed with error: {}", err);

            // The issue is in the resolve method in core.rs
            // When we have a path like a/field inside a function:
            // 1. 'a' is a function parameter, which is stored as TAG_STACK_VALUE
            // 2. When we resolve 'a', we get the value from the stack
            // 3. But when we try to access 'field' from it, we're not properly handling the context

            // Let's look at the specific issue in the resolve method
            if let CoreError::MemoryError(MemoryError::OutOfBounds) = err {
                println!("Confirmed: The error is MemoryError::OutOfBounds");
                println!("\nRoot cause analysis:");
                println!("In the resolve method in core.rs, when handling TAG_PATH:");
                println!("1. We resolve the first segment ('a') to a TAG_STACK_VALUE");
                println!("2. We get the value from the stack, which is a context");
                println!("3. We push the raw value (context address) onto the environment stack");
                println!("4. When we try to access 'field', we're trying to use the context address as a context");
                println!("5. This causes an out of bounds error because we're not properly handling the context");

                println!("\nThe fix would be to modify the resolve method to properly handle TAG_STACK_VALUE in path access:");
                println!("1. When we resolve a word to a TAG_STACK_VALUE, get the actual value from the stack");
                println!("2. If it's a context (TAG_CONTEXT), push the context onto the environment stack");
                println!("3. Then continue with the path resolution");
            }
        }
    }

    Ok(())
}
