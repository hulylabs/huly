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
    // Get command and host arguments from the stack
    let command = get_string_arg(module)?;
    let host = get_string_arg(module)?;

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

    // 3. If still not authenticated, return an error
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
    module.add_native_fn("ssh", ssh, 2)?;
    // Add more SSH-related functions in the future
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
