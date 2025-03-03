# SSH Functionality in Rebel

This document describes the SSH functionality available in Rebel, which allows you to execute commands on remote hosts via SSH.

## Basic Usage

The `ssh` function allows you to execute commands on remote hosts via SSH. It takes two arguments:

1. `host`: A string specifying the host to connect to, in the format `"[user@]hostname[:port]"`
2. `command`: A string containing the command to execute on the remote host

The function returns a context with the following fields:

- `stdout`: The standard output from the command
- `stderr`: The standard error from the command
- `exit_code`: The exit code of the command (0 typically indicates success)
- `success`: A boolean indicating if the command succeeded (exit code 0)

### Example

```
; Execute 'ls -la' on localhost
result: ssh "localhost" "ls -la"

; Print the results
print "Exit code:" result/exit_code
print "Success:" result/success
print "Output:" result/stdout
print "Errors:" result/stderr
```

## Host Format

The host string can be specified in several formats:

1. `"hostname"` - Connect to the specified hostname using the current user and default SSH port (22)
2. `"user@hostname"` - Connect to the specified hostname with the specified user and default SSH port (22)
3. `"hostname:port"` - Connect to the specified hostname and port using the current user
4. `"user@hostname:port"` - Connect to the specified hostname and port with the specified user

## Authentication

The SSH function tries several authentication methods in the following order:

1. SSH agent authentication - If an SSH agent is running, it will try to use it for authentication
2. Public key authentication - It will try to use the default SSH keys (`~/.ssh/id_rsa` and `~/.ssh/id_ed25519`)

If none of these methods succeed, the function will return an error.

## Examples

### Basic Example

```
; Connect to localhost and run 'ls -la'
result: ssh "localhost" "ls -la"
```

### Using a Specific Username

```
; Connect to example.com as 'user' and run 'whoami'
result: ssh "user@example.com" "whoami"
```

### Using a Specific Port

```
; Connect to example.com on port 2222 and run 'echo hello'
result: ssh "example.com:2222" "echo hello"
```

### Running Multiple Commands

```
; Run multiple commands separated by '&&'
result: ssh "localhost" "echo 'First command' && ls -la && echo 'Last command'"
```

## Error Handling

If the SSH connection or command execution fails, the function will return an error. You can use the `try` function to handle errors:

```
result: try [
    ssh "example.com" "ls -la"
]

either error? result [
    print "SSH failed:" result
][
    print "SSH succeeded:" result/stdout
]
```

## Future Enhancements

Future versions may include additional SSH-related functions such as:

- `scp` - Copy files to/from remote hosts
- `sftp` - SFTP file transfer functionality
- `ssh-tunnel` - Create SSH tunnels for port forwarding
