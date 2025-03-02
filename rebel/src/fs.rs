// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Exec, Module, VmValue};
use crate::mem::Word;
use crate::value::Value;
use std::fs;
use std::time::UNIX_EPOCH;

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

    // Create a vector to hold file contexts
    let mut files = Vec::new();

    // Process each directory entry
    for entry_result in entries {
        match entry_result {
            Ok(entry) => {
                // Get file metadata
                if let Ok(metadata) = entry.metadata() {
                    // Get filename
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy().to_string();

                    // Create a context for this file
                    let mut file_pairs = Vec::new();

                    // Add name
                    file_pairs.push(("name".into(), Value::String(name_str.into())));

                    // Add size
                    file_pairs.push(("size".into(), Value::Int(metadata.len() as i32)));

                    // Add file type
                    file_pairs.push(("is_dir".into(), Value::boolean(metadata.is_dir())));
                    file_pairs.push(("is_file".into(), Value::boolean(metadata.is_file())));

                    // Add modification time if available
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                            file_pairs
                                .push(("modified".into(), Value::Int(duration.as_secs() as i32)));
                        }
                    }

                    // Add creation time if available
                    if let Ok(created) = metadata.created() {
                        if let Ok(duration) = created.duration_since(UNIX_EPOCH) {
                            file_pairs
                                .push(("created".into(), Value::Int(duration.as_secs() as i32)));
                        }
                    }

                    // Add permissions
                    let permissions = metadata.permissions();
                    file_pairs.push(("readonly".into(), Value::boolean(permissions.readonly())));

                    // Create the file context
                    let file_context = Value::Context(file_pairs.into_boxed_slice());
                    files.push(file_context);
                }
            }
            Err(_) => continue, // Skip entries that can't be read
        }
    }

    // Create a block containing all file contexts
    let files_block = Value::Block(files.into_boxed_slice());

    // Convert the Value to a VmValue using the alloc_value method
    let vm_value = module.alloc_value(&files_block)?;

    // Push the VmValue onto the stack
    module.push(vm_value.vm_repr()).map_err(Into::into)
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

    // Create a string value
    let cwd_value = Value::String(cwd_str.into());

    // Convert the Value to a VmValue using the alloc_value method
    let vm_value = module.alloc_value(&cwd_value)?;

    // Push the VmValue onto the stack
    module.push(vm_value.vm_repr()).map_err(Into::into)
}

/// Change the current working directory
fn cd<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the directory path from the stack
    let [tag, data] = module.pop()?;
    let vm_value = VmValue::from_tag_data(tag, data)?;
    let value = module.to_value(vm_value)?;

    // Extract the path string
    let path = match value {
        Value::String(s) => s.to_string(),
        _ => return Err(CoreError::BadArguments),
    };

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
    let [tag, data] = module.pop()?;
    let vm_value = VmValue::from_tag_data(tag, data)?;
    let value = module.to_value(vm_value)?;

    // Extract the path string
    let path = match value {
        Value::String(s) => s.to_string(),
        _ => return Err(CoreError::BadArguments),
    };

    // Read the file contents
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Err(CoreError::InternalError),
    };

    // Create a string value
    let contents_value = Value::String(contents.into());

    // Convert the Value to a VmValue using the alloc_value method
    let vm_value = module.alloc_value(&contents_value)?;

    // Push the VmValue onto the stack
    module.push(vm_value.vm_repr()).map_err(Into::into)
}

/// Create a new directory
fn mkdir<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the directory path from the stack
    let [tag, data] = module.pop()?;
    let vm_value = VmValue::from_tag_data(tag, data)?;
    let value = module.to_value(vm_value)?;

    // Extract the path string
    let path = match value {
        Value::String(s) => s.to_string(),
        _ => return Err(CoreError::BadArguments),
    };

    // Create the directory
    if let Err(_) = std::fs::create_dir(&path) {
        return Err(CoreError::InternalError);
    }

    // Return a simple integer value (1 for success)
    module.push([VmValue::TAG_INT, 1]).map_err(Into::into)
}

/// Remove a file or directory
fn rm<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the path from the stack
    let [tag, data] = module.pop()?;
    let vm_value = VmValue::from_tag_data(tag, data)?;
    let value = module.to_value(vm_value)?;

    // Extract the path string
    let path = match value {
        Value::String(s) => s.to_string(),
        _ => return Err(CoreError::BadArguments),
    };

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

    // Return a simple integer value (1 for success)
    module.push([VmValue::TAG_INT, 1]).map_err(Into::into)
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
