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
    let entries = fs::read_dir(".")?;

    // Process each directory entry and collect into a vector
    let mut files = Vec::new();

    for entry_result in entries {
        let entry = entry_result?;
        let metadata = entry.metadata()?;

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

        files.push(file_ctx.build());
    }

    // Create a block containing all file contexts and push it onto the stack
    module.push_value(Value::block(files))
}

/// Print the current working directory
fn pwd<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the current working directory
    let cwd = std::env::current_dir()?;

    // Convert the path to a string
    let cwd_str = cwd.to_str().ok_or_else(|| {
        anyhow::anyhow!("Failed to convert path to string: non-UTF8 characters in path")
    })?;

    // Create a string value and push it onto the stack
    module.push_value(Value::string(cwd_str))
}

/// Change the current working directory
fn cd<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the directory path from the stack
    let path = get_string_arg(module)?;

    // Change the current directory
    std::env::set_current_dir(&path)?;

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
    let contents = std::fs::read_to_string(&path)?;

    // Create a string value and push it onto the stack
    module.push_value(Value::string(contents))
}

/// Create a new directory
fn mkdir<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the directory path from the stack
    let path = get_string_arg(module)?;

    // Create the directory
    std::fs::create_dir(&path)?;

    // Return a boolean value (true for success)
    module.push_value(Value::boolean(true))
}

/// Remove a file or directory
fn rm<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get the path from the stack
    let path = get_string_arg(module)?;

    // Check if it's a directory or a file
    let metadata = std::fs::metadata(&path)?;

    // Remove the file or directory
    if metadata.is_dir() {
        std::fs::remove_dir(&path)?;
    } else {
        std::fs::remove_file(&path)?;
    }

    // Return a boolean value (true for success)
    module.push_value(Value::boolean(true))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Exec, Module};
    use crate::mem::Word;
    use crate::value::Value;
    use std::fs;
    use std::path::Path; // Needed for Path::new() and canonicalize()
    use tempfile::TempDir;

    // Helper function to create a module and execution context
    fn setup_module() -> Module<Box<[Word]>> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Register filesystem functions
        fs_package(&mut module).expect("Failed to register filesystem functions");

        module
    }

    // Helper function to create an execution context from a module
    fn setup_exec(module: &mut Module<Box<[Word]>>) -> Exec<Box<[Word]>> {
        // Create a dummy block for the execution context
        let dummy_block = module
            .alloc_value(&Value::block(vec![]))
            .expect("Failed to allocate empty block");

        // Create an execution context
        module
            .new_process(dummy_block)
            .expect("Failed to create execution context")
    }

    #[test]
    fn test_pwd() {
        let mut module = setup_module();
        let mut exec = setup_exec(&mut module);

        // Call the pwd function
        pwd(&mut exec).expect("Failed to call pwd");

        // Get the result
        let result = exec.pop_to_value().expect("Failed to get result");

        // Verify the result is a string
        assert!(result.is_string(), "pwd should return a string");

        // Verify the result matches the current directory
        let current_dir = std::env::current_dir()
            .expect("Failed to get current directory")
            .to_string_lossy()
            .to_string();

        if let Value::String(s) = result {
            assert_eq!(s, current_dir, "pwd should return the current directory");
        }
    }

    #[test]
    fn test_ls() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let temp_path = temp_dir.path();

        // Create some test files in the temporary directory
        let test_file = temp_path.join("test_file.txt");
        fs::write(&test_file, "test content").expect("Failed to write test file");

        let test_dir = temp_path.join("test_dir");
        fs::create_dir(&test_dir).expect("Failed to create test directory");

        // Change to the temporary directory
        let original_dir = std::env::current_dir().expect("Failed to get current directory");
        std::env::set_current_dir(temp_path).expect("Failed to change directory");

        // Create module and execution context
        let mut module = setup_module();
        let mut exec = setup_exec(&mut module);

        // Call the ls function
        ls(&mut exec).expect("Failed to call ls");

        // Get the result
        let result = exec.pop_to_value().expect("Failed to get result");

        // Verify the result is a block
        assert!(result.is_block(), "ls should return a block");

        // Verify the block contains our test files
        if let Value::Block(files) = result {
            // There should be at least 2 entries (our test file and directory)
            assert!(files.len() >= 2, "ls should return at least 2 entries");

            // Check if our test files are in the result
            let has_test_file = files.iter().any(|file| {
                if let Value::Context(pairs) = file {
                    pairs.iter().any(|(key, value)| {
                        key == "name" && value == &Value::string("test_file.txt")
                    })
                } else {
                    false
                }
            });

            let has_test_dir = files.iter().any(|file| {
                if let Value::Context(pairs) = file {
                    pairs
                        .iter()
                        .any(|(key, value)| key == "name" && value == &Value::string("test_dir"))
                } else {
                    false
                }
            });

            assert!(has_test_file, "ls result should contain test_file.txt");
            assert!(has_test_dir, "ls result should contain test_dir");
        }

        // Change back to the original directory
        std::env::set_current_dir(original_dir)
            .expect("Failed to change back to original directory");
    }

    #[test]
    fn test_cd() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let temp_path = temp_dir.path();

        // Get the original directory
        let original_dir = std::env::current_dir().expect("Failed to get current directory");

        // Create module and execution context
        let mut module = setup_module();
        let mut exec = setup_exec(&mut module);

        // Push the temporary directory path as an argument
        exec.push_value(Value::string(temp_path.to_string_lossy().to_string()))
            .expect("Failed to push directory path");

        // Call the cd function
        cd(&mut exec).expect("Failed to call cd");

        // Get the result
        let result = exec.pop_to_value().expect("Failed to get result");

        // Verify the result is a string
        assert!(result.is_string(), "cd should return a string");

        // Verify the current directory has changed
        let current_dir = std::env::current_dir().expect("Failed to get current directory");

        // On macOS, /tmp is a symlink to /private/tmp, so we need to canonicalize the paths
        let canonical_current = current_dir
            .canonicalize()
            .expect("Failed to canonicalize current dir");
        let canonical_temp = temp_path
            .canonicalize()
            .expect("Failed to canonicalize temp dir");

        assert_eq!(
            canonical_current, canonical_temp,
            "Current directory should be the temporary directory"
        );

        // Verify the result matches the new current directory
        if let Value::String(s) = result {
            // Convert the string path to a Path and canonicalize it for comparison
            let result_path = Path::new(&s);
            let result_canonical = result_path
                .canonicalize()
                .expect("Failed to canonicalize result path");

            assert_eq!(
                result_canonical, canonical_temp,
                "cd should return the new current directory"
            );
        }

        // Change back to the original directory
        std::env::set_current_dir(original_dir)
            .expect("Failed to change back to original directory");
    }

    #[test]
    fn test_cat() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let temp_path = temp_dir.path();

        // Create a test file with known content
        let test_content = "This is a test file content for cat function";
        let test_file = temp_path.join("test_cat.txt");
        fs::write(&test_file, test_content).expect("Failed to write test file");

        // Create module and execution context
        let mut module = setup_module();
        let mut exec = setup_exec(&mut module);

        // Push the test file path as an argument
        exec.push_value(Value::string(test_file.to_string_lossy().to_string()))
            .expect("Failed to push file path");

        // Call the cat function
        cat(&mut exec).expect("Failed to call cat");

        // Get the result
        let result = exec.pop_to_value().expect("Failed to get result");

        // Verify the result is a string
        assert!(result.is_string(), "cat should return a string");

        // Verify the result matches the file content
        if let Value::String(s) = result {
            assert_eq!(s, test_content, "cat should return the file content");
        }
    }

    #[test]
    fn test_mkdir() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let temp_path = temp_dir.path();

        // Define a new directory to create
        let new_dir = temp_path.join("new_test_dir");

        // Create module and execution context
        let mut module = setup_module();
        let mut exec = setup_exec(&mut module);

        // Push the new directory path as an argument
        exec.push_value(Value::string(new_dir.to_string_lossy().to_string()))
            .expect("Failed to push directory path");

        // Call the mkdir function
        mkdir(&mut exec).expect("Failed to call mkdir");

        // Get the result
        let result = exec.pop_to_value().expect("Failed to get result");

        // Verify the result is a boolean true
        assert!(result.is_boolean(), "mkdir should return a boolean");
        assert_eq!(result, Value::boolean(true), "mkdir should return true");

        // Verify the directory was created
        assert!(new_dir.exists(), "Directory should exist after mkdir");
        assert!(new_dir.is_dir(), "Created path should be a directory");
    }

    #[test]
    fn test_rm() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let temp_path = temp_dir.path();

        // Create a test file to remove
        let test_file = temp_path.join("test_rm_file.txt");
        fs::write(&test_file, "test content").expect("Failed to write test file");

        // Create a test directory to remove
        let test_dir = temp_path.join("test_rm_dir");
        fs::create_dir(&test_dir).expect("Failed to create test directory");

        // Test removing a file
        {
            let mut module = setup_module();
            let mut exec = setup_exec(&mut module);

            // Push the test file path as an argument
            exec.push_value(Value::string(test_file.to_string_lossy().to_string()))
                .expect("Failed to push file path");

            // Call the rm function
            rm(&mut exec).expect("Failed to call rm on file");

            // Get the result
            let result = exec.pop_to_value().expect("Failed to get result");

            // Verify the result is a boolean true
            assert!(result.is_boolean(), "rm should return a boolean");
            assert_eq!(result, Value::boolean(true), "rm should return true");

            // Verify the file was removed
            assert!(!test_file.exists(), "File should not exist after rm");
        }

        // Test removing a directory
        {
            let mut module = setup_module();
            let mut exec = setup_exec(&mut module);

            // Push the test directory path as an argument
            exec.push_value(Value::string(test_dir.to_string_lossy().to_string()))
                .expect("Failed to push directory path");

            // Call the rm function
            rm(&mut exec).expect("Failed to call rm on directory");

            // Get the result
            let result = exec.pop_to_value().expect("Failed to get result");

            // Verify the result is a boolean true
            assert!(result.is_boolean(), "rm should return a boolean");
            assert_eq!(result, Value::boolean(true), "rm should return true");

            // Verify the directory was removed
            assert!(!test_dir.exists(), "Directory should not exist after rm");
        }
    }
}
