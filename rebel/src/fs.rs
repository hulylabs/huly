// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{Exec, Value, inline_string};
use crate::mem::Word;
use crate::module::Module;
use std::fs;

/// Implementation of the 'ls' command to list files in a directory
fn ls_command<T, B>(exec: &mut Exec<T, B>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
    B: crate::module::BlobStore,
{
    // Get current directory or use provided path argument
    // Currently only supports current directory, will be extended to use arguments
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    // Read directory entries
    let entries = match fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(_) => return None,
    };

    // Create a string to hold directory listing
    let mut result = String::new();
    
    // Process directory entries
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        
        // Check if it's a directory and append slash if needed
        if path.is_dir() {
            result.push_str(&format!("{}/\n", name));
        } else {
            result.push_str(&format!("{}\n", name));
        }
    }

    // Create an inline string for results that fit
    if let Some(inline_result) = inline_string(&result) {
        let offset = exec.alloc(inline_result)?;
        exec.push([Value::TAG_INLINE_STRING, offset])
    } else {
        // For larger strings, just return an empty string for now
        // In a full implementation, we would need to implement block allocation
        // for larger strings
        if let Some(empty) = inline_string("") {
            let offset = exec.alloc(empty)?;
            exec.push([Value::TAG_INLINE_STRING, offset])
        } else {
            // This shouldn't ever happen (empty string will always fit)
            exec.push([Value::TAG_NONE, 0])
        }
    }
}

/// Register filesystem-related commands with a module
pub fn register_fs_commands<T, B>(module: &mut Module<T, B>) -> Option<()>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
    B: crate::module::BlobStore,
{
    // Register the ls command (0 arguments for now)
    module.add_native_fn("ls", ls_command, 0)?;
    
    // Future expansion: Add more filesystem commands here
    // module.add_native_fn("cat", cat_command, 1)?;
    // module.add_native_fn("mkdir", mkdir_command, 1)?;
    // etc.
    
    Some(())
}