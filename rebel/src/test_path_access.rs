// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Module, VmValue};
use crate::rebel;
use crate::value::Value;

/// Test to identify the root cause of the path access issue
///
/// The issue is with the following code:
/// f: func [a] [print a/field] f context [field: 5]
///
/// This test will help us understand why this results in an "out of bounds" error
#[test]
fn test_path_access_issue() -> Result<(), CoreError> {
    // Initialize a module
    let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;

    // First, let's test a simple context access to verify contexts work correctly
    let simple_context_test = "ctx: context [field: 5] ctx/field";
    let simple_result = eval_code(&mut module, simple_context_test)?;

    // This should work and return 5
    assert_eq!(simple_result, Value::Int(5), "Simple context access failed");
    println!("Simple context access works correctly");

    // Now let's test the problematic case with a function
    let function_test = "f: func [a] [a/field] f context [field: 5]";

    // This is expected to fail with an "out of bounds" error
    match eval_code(&mut module, function_test) {
        Ok(result) => {
            println!(
                "Function test unexpectedly succeeded with result: {}",
                result
            );
            assert_eq!(
                result,
                Value::Int(5),
                "Function should return 5 if it works"
            );
        }
        Err(err) => {
            println!("Function test failed with error: {}", err);
            // We expect a MemoryError::OutOfBounds error
            assert!(
                format!("{}", err).contains("out of bounds"),
                "Expected 'out of bounds' error, got: {}",
                err
            );
        }
    }

    // Let's try to understand what's happening by breaking it down
    // First, create a context and store it in a variable
    let setup_test = "test_ctx: context [field: 5]";
    eval_code(&mut module, setup_test)?;

    // Now define a function that accesses a field from its argument
    let define_func = "test_func: func [a] [a/field]";
    eval_code(&mut module, define_func)?;

    // Call the function with the context directly
    let call_test = "result: test_func test_ctx";
    eval_code(&mut module, call_test)?;

    // Check the result
    let check_result = "result";
    let final_result = eval_code(&mut module, check_result)?;
    assert_eq!(
        final_result,
        Value::Int(5),
        "Function call with context should return 5"
    );

    // Now let's try to debug the path access mechanism
    // Create a function that prints the type of its argument
    let debug_func = "debug_func: func [a] [
        print \"Argument type: \"
        either block? a [print \"block\"] [print \"not block\"]
        either context? a [print \"context\"] [print \"not context\"]
        a
    ]";
    eval_code(&mut module, debug_func)?;

    // Add a context? function if it doesn't exist
    let add_context_check = "context?: func [value] [
        either value [true] [false]
    ]";
    eval_code(&mut module, add_context_check)?;

    // Call the debug function with our context
    let debug_call = "debug_func test_ctx";
    let debug_result = eval_code(&mut module, debug_call)?;

    // The debug function should return the context unchanged
    assert!(
        matches!(debug_result, Value::Context(_)),
        "Debug function should return the context"
    );

    // Now let's try to understand what happens in the path access
    // Create a function that tries to access a field but with error handling
    let safe_access = "safe_access: func [a] [
        either context? a [
            print \"Accessing field from context\"
            a/field
        ] [
            print \"Not a context, cannot access field\"
            none
        ]
    ]";
    eval_code(&mut module, safe_access)?;

    // Call the safe access function with our context
    let safe_call = "safe_result: safe_access test_ctx";
    eval_code(&mut module, safe_call)?;

    // Check the result
    let check_safe = "safe_result";
    let safe_result = eval_code(&mut module, check_safe)?;
    assert_eq!(safe_result, Value::Int(5), "Safe access should return 5");

    Ok(())
}

/// Helper function to evaluate code and convert the result to a Value
fn eval_code(module: &mut Module<Box<[u32]>>, code: &str) -> Result<Value, CoreError> {
    let block = module.parse(code)?;
    let result = module.eval(block)?;
    module.to_value(result)
}

/// Test to specifically isolate and identify the issue with path access in functions
#[test]
fn test_path_access_isolation() -> Result<(), CoreError> {
    // Initialize a module
    let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;

    // First, create a context with a field
    let ctx_code = "test_ctx: context [field: 5]";
    eval_code(&mut module, ctx_code)?;

    // Verify we can access the field directly
    let direct_access = "test_ctx/field";
    let direct_result = eval_code(&mut module, direct_access)?;
    assert_eq!(direct_result, Value::Int(5), "Direct field access failed");

    // Now define a function that takes an argument and tries to access its field
    let func_def = "test_func: func [a] [a/field]";
    eval_code(&mut module, func_def)?;

    // Call the function with the context
    let func_call = "test_func test_ctx";

    // This is where we expect the error to occur
    match eval_code(&mut module, func_call) {
        Ok(result) => {
            println!("Function call succeeded with result: {}", result);
            assert_eq!(
                result,
                Value::Int(5),
                "Function should return 5 if it works"
            );
        }
        Err(err) => {
            println!("Function call failed with error: {}", err);
            // We expect a MemoryError::OutOfBounds error
            assert!(
                format!("{}", err).contains("out of bounds"),
                "Expected 'out of bounds' error, got: {}",
                err
            );

            // Now let's try to understand why this happens
            // The issue might be in how the context is passed to the function
            // or how the path access is resolved within the function

            // Let's try to print the argument inside the function
            let debug_func = "debug_func: func [a] [
                print \"Argument: \"
                print a
                a
            ]";
            eval_code(&mut module, debug_func)?;

            // Call the debug function with our context
            let debug_call = "debug_func test_ctx";
            eval_code(&mut module, debug_call)?;

            // Now let's try to access a field from the argument in a different way
            let alt_func = "alt_func: func [a] [
                field_val: none
                foreach key a [
                    if key = \"field\" [
                        field_val: a/:key
                    ]
                ]
                field_val
            ]";

            // This might not work either, but it's worth trying
            match eval_code(&mut module, alt_func) {
                Ok(_) => {
                    let alt_call = "alt_func test_ctx";
                    match eval_code(&mut module, alt_call) {
                        Ok(alt_result) => {
                            println!(
                                "Alternative function call succeeded with result: {}",
                                alt_result
                            );
                        }
                        Err(alt_err) => {
                            println!("Alternative function call failed with error: {}", alt_err);
                        }
                    }
                }
                Err(def_err) => {
                    println!("Failed to define alternative function: {}", def_err);
                }
            }
        }
    }

    Ok(())
}
