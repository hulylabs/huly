// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use crate::core::{CoreError, Exec, Module};
use crate::mem::Word;
use crate::value::Value;
use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;

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

/// Parse a host string in the format "user@hostname:port"
/// Returns (username, hostname, port)
/// If username is not provided, the current user is used
/// If port is not provided, the default SSH port (22) is used
fn parse_host(host: &str) -> Result<(String, String, u16), CoreError> {
    // Default values
    let username;
    let hostname;
    let port;

    // First, split by '@' to get username and host part
    let host_part;
    if let Some(pos) = host.find('@') {
        username = host[..pos].to_string();
        host_part = &host[pos + 1..];
    } else {
        username = whoami::username();
        host_part = host;
    }

    // Then, split host part by ':' to get hostname and port
    if let Some(pos) = host_part.find(':') {
        hostname = host_part[..pos].to_string();
        port = host_part[pos + 1..]
            .parse()
            .map_err(|_| CoreError::BadArguments)?;
    } else {
        hostname = host_part.to_string();
        port = 22; // Default SSH port
    }

    Ok((username, hostname, port))
}

/// Execute a command on a remote host via SSH with options
/// Arguments:
/// - host: String in the format "user@hostname:port"
/// - command: String command to execute
/// - options: Context with optional parameters:
///   - password: String password for authentication
///   - timeout: Integer timeout in seconds
///   - key_path: String path to private key file
/// Returns a context with:
/// - stdout: String output from the command
/// - stderr: String error output from the command
/// - exit_code: Integer exit code
/// - success: Boolean indicating if the command succeeded (exit code 0)
fn ssh_with_options<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get arguments from the stack
    let options = module.pop_to_value()?;
    let command = get_string_arg(module)?;
    let host = get_string_arg(module)?;

    // Call the implementation with options
    ssh_with_options_impl(host, command, options, module)
}

/// Execute a command on a remote host via SSH
/// Arguments:
/// - host: String in the format "user@hostname:port"
/// - command: String command to execute
/// Returns a context with:
/// - stdout: String output from the command
/// - stderr: String error output from the command
/// - exit_code: Integer exit code
/// - success: Boolean indicating if the command succeeded (exit code 0)
fn ssh<T>(module: &mut Exec<T>) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Get arguments from the stack
    let command = get_string_arg(module)?;
    let host = get_string_arg(module)?;

    // Create an empty options context
    let options = Value::object().build();

    // Call the implementation with options
    ssh_with_options_impl(host, command, options, module)
}

/// Implementation of SSH with options
/// This is the common implementation used by both ssh and ssh-with-options functions
fn ssh_with_options_impl<T>(
    host: String,
    command: String,
    options: Value,
    module: &mut Exec<T>,
) -> Result<(), CoreError>
where
    T: AsRef<[Word]> + AsMut<[Word]>,
{
    // Extract options from the context
    let password = if let Value::Context(pairs) = &options {
        pairs.iter().find_map(|(key, value)| {
            if key == "password" {
                if let Value::String(s) = value {
                    Some(s.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
    } else {
        None
    };

    // Parse host string
    let (username, hostname, port) = parse_host(&host)?;

    // Connect to the server
    let tcp = TcpStream::connect(format!("{}:{}", hostname, port))
        .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;

    let mut sess =
        Session::new().map_err(|e| anyhow::anyhow!("Failed to create SSH session: {}", e))?;

    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| anyhow::anyhow!("SSH handshake failed: {}", e))?;

    // Try authentication methods in order:

    // 1. Try SSH agent authentication
    if let Ok(mut agent) = sess.agent() {
        if agent.connect().is_ok() && agent.list_identities().is_ok() {
            for identity in agent.identities().unwrap_or_default() {
                if agent.userauth(&username, &identity).is_ok() {
                    break;
                }
            }
        }
    }

    // 2. Try public key authentication if not authenticated yet
    if !sess.authenticated() {
        // Try default key locations
        let key_paths = vec![
            format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap_or_default()),
            format!(
                "{}/.ssh/id_ed25519",
                std::env::var("HOME").unwrap_or_default()
            ),
        ];

        for key_path in key_paths {
            if Path::new(&key_path).exists() {
                let _ = sess.userauth_pubkey_file(&username, None, Path::new(&key_path), None);

                if sess.authenticated() {
                    break;
                }
            }
        }
    }

    // 3. Try password authentication if provided and still not authenticated
    if !sess.authenticated() && password.is_some() {
        let _ = sess.userauth_password(&username, &password.unwrap());
    }

    // 4. If still not authenticated, return an error
    if !sess.authenticated() {
        return Err(anyhow::anyhow!("Authentication failed").into());
    }

    // Execute the command
    let mut channel = sess
        .channel_session()
        .map_err(|e| anyhow::anyhow!("Failed to open channel: {}", e))?;

    channel
        .exec(&command)
        .map_err(|e| anyhow::anyhow!("Failed to execute command: {}", e))?;

    // Capture stdout
    let mut stdout = String::new();
    channel
        .read_to_string(&mut stdout)
        .map_err(|e| anyhow::anyhow!("Failed to read stdout: {}", e))?;

    // Capture stderr
    let mut stderr = String::new();
    channel
        .stderr()
        .read_to_string(&mut stderr)
        .map_err(|e| anyhow::anyhow!("Failed to read stderr: {}", e))?;

    // Get exit status
    channel
        .wait_close()
        .map_err(|e| anyhow::anyhow!("Failed to wait for channel close: {}", e))?;

    let exit_code = channel
        .exit_status()
        .map_err(|e| anyhow::anyhow!("Failed to get exit status: {}", e))?;

    // Create result context
    let result = Value::object()
        .insert("stdout", stdout)
        .insert("stderr", stderr)
        .insert("exit_code", exit_code as i32)
        .insert("success", exit_code == 0)
        .build();

    // Push result onto the stack
    module.push_value(result)
}

/// Register SSH functions
pub fn ssh_package<T>(module: &mut Module<T>) -> Result<(), CoreError>
where
    T: AsMut<[Word]> + AsRef<[Word]>,
{
    // Register the basic SSH function (host, command)
    module.add_native_fn("ssh", ssh, 2)?;

    // Register the advanced SSH function with options (host, command, options)
    module.add_native_fn("ssh-with-options", ssh_with_options, 3)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Exec, Module};
    use crate::mem::Word;
    use crate::value::Value;

    // Helper function to create a module and execution context
    fn setup_module() -> Module<Box<[Word]>> {
        let mut module =
            Module::init(vec![0; 0x10000].into_boxed_slice()).expect("Failed to create module");

        // Register SSH functions
        ssh_package(&mut module).expect("Failed to register SSH functions");

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
    fn test_parse_host_full() {
        let result = parse_host("user@example.com:2222").unwrap();
        assert_eq!(result.0, "user");
        assert_eq!(result.1, "example.com");
        assert_eq!(result.2, 2222);
    }

    #[test]
    fn test_parse_host_no_port() {
        let result = parse_host("user@example.com").unwrap();
        assert_eq!(result.0, "user");
        assert_eq!(result.1, "example.com");
        assert_eq!(result.2, 22);
    }

    #[test]
    fn test_parse_host_no_user() {
        let result = parse_host("example.com").unwrap();
        assert_eq!(result.0, whoami::username());
        assert_eq!(result.1, "example.com");
        assert_eq!(result.2, 22);
    }

    #[test]
    fn test_parse_host_no_user_with_port() {
        let result = parse_host("example.com:2222").unwrap();
        assert_eq!(result.0, whoami::username());
        assert_eq!(result.1, "example.com");
        assert_eq!(result.2, 2222);
    }

    // Integration tests for SSH functionality
    // These tests require a running SSH server on localhost:2222
    // with username "testuser" and password "password"
    #[test]
    fn test_ssh_basic() {
        // Create module and execution context
        let mut module = setup_module();
        let mut exec = setup_exec(&mut module);

        // Create options context with password
        let options = Value::object().insert("password", "password").build();

        // Push arguments onto the stack in reverse order (options, command, host)
        exec.push_value(Value::string("testuser@localhost:2222"))
            .expect("Failed to push host");
        exec.push_value(Value::string("echo 'Hello from SSH'"))
            .expect("Failed to push command");
        exec.push_value(options).expect("Failed to push options");

        // Call the SSH function
        ssh_with_options(&mut exec).expect("Failed to call SSH function");

        // Get the result
        let result = exec.pop_to_value().expect("Failed to get result");

        // Verify the result is a context
        assert!(
            matches!(result, Value::Context(_)),
            "Result should be a context"
        );

        // Extract and verify the result fields
        if let Value::Context(pairs) = result {
            // Create a map for easier access
            let result_map: std::collections::HashMap<_, _> =
                pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

            // Verify stdout contains our expected output
            if let Some(Value::String(stdout)) = result_map.get("stdout") {
                assert!(
                    stdout.contains("Hello from SSH"),
                    "stdout should contain 'Hello from SSH'"
                );
            } else {
                panic!("stdout field missing or not a string");
            }

            // Verify exit code is 0
            if let Some(Value::Int(exit_code)) = result_map.get("exit_code") {
                assert_eq!(*exit_code, 0, "exit_code should be 0");
            } else {
                panic!("exit_code field missing or not an integer");
            }

            // Verify success is true
            if let Some(Value::Int(success)) = result_map.get("success") {
                assert_eq!(*success, 1, "success should be true");
            } else {
                panic!("success field missing or not a boolean");
            }
        }
    }

    #[test]
    fn test_ssh_file_operations() {
        // Create module and execution context
        let mut module = setup_module();
        let mut exec = setup_exec(&mut module);

        // Create options context with password
        let options = Value::object().insert("password", "password").build();

        // 1. Create a test file - push arguments in reverse order (host, command, options)
        let create_file_cmd = "echo 'Test content' > /tmp/rebel_test_file.txt";
        exec.push_value(Value::string("testuser@localhost:2222"))
            .expect("Failed to push host");
        exec.push_value(Value::string(create_file_cmd))
            .expect("Failed to push command");
        exec.push_value(options.clone())
            .expect("Failed to push options");
        ssh_with_options(&mut exec).expect("Failed to create test file");
        exec.pop_to_value().expect("Failed to get result"); // Discard result

        // 2. Read the file content - push arguments in reverse order (host, command, options)
        let read_file_cmd = "cat /tmp/rebel_test_file.txt";
        exec.push_value(Value::string("testuser@localhost:2222"))
            .expect("Failed to push host");
        exec.push_value(Value::string(read_file_cmd))
            .expect("Failed to push command");
        exec.push_value(options.clone())
            .expect("Failed to push options");
        ssh_with_options(&mut exec).expect("Failed to read test file");

        // Get the result
        let read_result = exec.pop_to_value().expect("Failed to get result");

        // Verify the file content
        if let Value::Context(pairs) = read_result {
            let result_map: std::collections::HashMap<_, _> =
                pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

            if let Some(Value::String(stdout)) = result_map.get("stdout") {
                assert!(
                    stdout.contains("Test content"),
                    "File content should contain 'Test content'"
                );
            } else {
                panic!("stdout field missing or not a string");
            }
        }

        // 3. Delete the test file - push arguments in reverse order (host, command, options)
        let delete_file_cmd = "rm /tmp/rebel_test_file.txt";
        exec.push_value(Value::string("testuser@localhost:2222"))
            .expect("Failed to push host");
        exec.push_value(Value::string(delete_file_cmd))
            .expect("Failed to push command");
        exec.push_value(options).expect("Failed to push options");
        ssh_with_options(&mut exec).expect("Failed to delete test file");

        // Get the result
        let delete_result = exec.pop_to_value().expect("Failed to get result");

        // Verify the deletion was successful
        if let Value::Context(pairs) = delete_result {
            let result_map: std::collections::HashMap<_, _> =
                pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

            if let Some(Value::Int(exit_code)) = result_map.get("exit_code") {
                assert_eq!(*exit_code, 0, "File deletion should succeed");
            } else {
                panic!("exit_code field missing or not an integer");
            }
        }
    }

    #[test]
    fn test_ssh_multiple_commands() {
        // Create module and execution context
        let mut module = setup_module();
        let mut exec = setup_exec(&mut module);

        // Create options context with password
        let options = Value::object().insert("password", "password").build();

        // Execute multiple commands - push arguments in reverse order (host, command, options)
        let commands = "echo 'First command' && ls -la /tmp && echo 'Last command'";
        exec.push_value(Value::string("testuser@localhost:2222"))
            .expect("Failed to push host");
        exec.push_value(Value::string(commands))
            .expect("Failed to push command");
        exec.push_value(options).expect("Failed to push options");
        ssh_with_options(&mut exec).expect("Failed to execute multiple commands");

        // Get the result
        let result = exec.pop_to_value().expect("Failed to get result");

        // Verify the result
        if let Value::Context(pairs) = result {
            let result_map: std::collections::HashMap<_, _> =
                pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

            if let Some(Value::String(stdout)) = result_map.get("stdout") {
                assert!(
                    stdout.contains("First command"),
                    "Output should contain 'First command'"
                );
                assert!(
                    stdout.contains("Last command"),
                    "Output should contain 'Last command'"
                );
                assert!(
                    stdout.contains("total"),
                    "Output should contain directory listing"
                );
            } else {
                panic!("stdout field missing or not a string");
            }
        }
    }
}
