// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Exec, Module};
use crate::mem::Word;
use crate::value::Value;
use std::fs;
use std::time::UNIX_EPOCH;

/// Helper function to extract a string argument from the stack
fn get_string_arg<T>(module: &mut Exec<T>) -> Result<String, CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    let value = module.pop_to_value()?;
    match value {
        Value::String(s) => Ok(s.to_string()),
        _ => Err(CoreError::BadArguments),
    }
}

/// List files in the current directory
/// Returns a block of contexts, each representing a file with its metadata
fn ls<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get current directory entries
    let entries = match fs::read_dir(".") {
        Ok(entries) => entries,
        Err(_) => return Err(CoreError::InternalError),
    };

    // Process each directory entry and collect into a vector
    let files = entries
        .filter_map(|entry_result| {
            let entry = entry_result.ok()?;
            let metadata = entry.metadata().ok()?;

            // Get filename
            let name = entry.file_name();
            let name_str = name.to_string_lossy().to_string();

            // Create a context for this file using Value::object()
            let mut file_ctx = Value::object()
                .insert("name", name_str)
                .insert("size", metadata.len() as i32)
                .insert("is_dir", metadata.is_dir())
                .insert("is_file", metadata.is_file())
                .insert("readonly", metadata.permissions().readonly());

            // Add modification time if available
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    file_ctx = file_ctx.insert("modified", duration.as_secs() as i32);
                }
            }

            // Add creation time if available
            if let Ok(created) = metadata.created() {
                if let Ok(duration) = created.duration_since(UNIX_EPOCH) {
                    file_ctx = file_ctx.insert("created", duration.as_secs() as i32);
                }
            }

            Some(file_ctx.build())
        })
        .collect::<Vec<_>>();

    // Create a block containing all file contexts and push it onto the stack
    module.push_value_direct(Value::block(files))
}

/// Print the current working directory
fn pwd<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the current working directory
    let cwd = match std::env::current_dir() {
        Ok(path) => path,
        Err(_) => return Err(CoreError::InternalError),
    };

    // Convert the path to a string
    let cwd_str = match cwd.to_str() {
        Some(s) => s,
        None => return Err(CoreError::InternalError),
    };

    // Create a string value and push it onto the stack
    module.push_value_direct(Value::string(cwd_str))
}

/// Change the current working directory
fn cd<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the directory path from the stack
    let path = get_string_arg(module)?;

    // Change the current directory
    if let Err(_) = std::env::set_current_dir(&path) {
        return Err(CoreError::InternalError);
    }

    // Return the new current directory
    pwd(module)
}

/// Read the contents of a file
fn cat<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the file path from the stack
    let path = get_string_arg(module)?;

    // Read the file contents
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Err(CoreError::InternalError),
    };

    // Create a string value and push it onto the stack
    module.push_value_direct(Value::string(contents))
}

/// Create a new directory
fn mkdir<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the directory path from the stack
    let path = get_string_arg(module)?;

    // Create the directory
    if let Err(_) = std::fs::create_dir(&path) {
        return Err(CoreError::InternalError);
    }

    // Return a boolean value (true for success)
    module.push_value_direct(Value::boolean(true))
}

/// Remove a file or directory
fn rm<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the path from the stack
    let path = get_string_arg(module)?;

    // Check if it's a directory or a file
    let metadata = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(_) => return Err(CoreError::InternalError),
    };

    // Remove the file or directory
    if metadata.is_dir() {
        if let Err(_) = std::fs::remove_dir(&path) {
            return Err(CoreError::InternalError);
        }
    } else {
        if let Err(_) = std::fs::remove_file(&path) {
            return Err(CoreError::InternalError);
        }
    }

    // Return a boolean value (true for success)
    module.push_value_direct(Value::boolean(true))
}

/// Register all filesystem functions
pub fn fs_package<T>(module: &mut Module<T>) -> Result<(), CoreError>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    module.add_native_fn("ls", ls, 0)?;
    module.add_native_fn("pwd", pwd, 0)?;
    module.add_native_fn("cd", cd, 1)?;
    module.add_native_fn("cat", cat, 1)?;
    module.add_native_fn("mkdir", mkdir, 1)?;
    module.add_native_fn("rm", rm, 1)?;
    // Add more filesystem functions in the future
    Ok(())
}
