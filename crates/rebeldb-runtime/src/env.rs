// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// env.rs:

pub struct Environment {
    parent: *mut Environment,
    // vars: *mut VarMap, // Could be your hashmap implementation
}

#[no_mangle]
pub extern "C" fn env_lookup(env_ptr: *mut Environment, var_id: i32) -> i32 {
    let mut current = env_ptr;

    unsafe {
        while !current.is_null() {
            let env = &*current;

            // Look up in current environment
            // if let Some(value) = (*env.vars).get(var_id) {
            //     return *value;
            // }

            // Move up the chain
            current = env.parent;
        }

        // Not found - you might want different error handling
        -1
    }
}

#[no_mangle]
pub extern "C" fn env_create(parent: *mut Environment) -> *mut Environment {
    // Allocate new environment
    // Note: You'll need to handle allocation carefully
    // Maybe use your arena allocator here
    let env = Box::new(Environment {
        parent,
        // vars: Box::into_raw(Box::new(VarMap::new())),
    });

    Box::into_raw(env)
}
