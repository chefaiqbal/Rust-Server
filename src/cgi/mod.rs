use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::io::Write;
use std::path::Path;
use std::os::unix::io::{AsRawFd, RawFd};
use libc::{fcntl, F_SETFL, O_NONBLOCK};

pub struct CgiHandler {
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct CgiRequest {
    pub script_path: String,
    pub method: String,
    pub uri: String,
    pub query_string: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub remote_addr: String,
}

#[derive(Debug)]
pub struct CgiResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub struct CgiProcess {
    pub child: std::process::Child,
    pub stdin_fd: Option<RawFd>,
    pub stdout_fd: Option<RawFd>,
    pub stderr_fd: Option<RawFd>,
}

impl CgiHandler {
    pub fn new() -> Self {
        Self {
            timeout_seconds: 30,
        }
    }

    pub fn execute(&self, request: CgiRequest) -> Result<CgiResponse, Box<dyn std::error::Error>> {
        if !Path::new(&request.script_path).exists() {
            return Err("CGI script not found".into());
        }

        let env_vars = self.build_environment(&request);
        
        let mut child = Command::new(&request.script_path)
            .envs(&env_vars)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // --- NON-BLOCKING CGI I/O SUGGESTION ---
        // To be fully non-blocking and epoll-compliant:
        // 1. Set child.stdin, child.stdout, and child.stderr to non-blocking mode using libc::fcntl.
        //    Example:
        //    use std::os::unix::io::AsRawFd;
        //    use libc::{fcntl, F_SETFL, O_NONBLOCK};
        //    let fd = child.stdout.as_ref().unwrap().as_raw_fd();
        //    unsafe { fcntl(fd, F_SETFL, O_NONBLOCK); }
        // 2. Register these fds with your epoll manager.
        // 3. Integrate CGI I/O into your event loop, reading/writing only when epoll signals readiness.
        // 4. Avoid wait_with_output (which is blocking); instead, poll for process completion and I/O readiness.
        //
        // For now, the following is blocking and should be refactored for full compliance:

        // Write request body to stdin if present
        if !request.body.is_empty() {
            if let Some(stdin) = child.stdin.as_mut() {
                match stdin.write_all(&request.body) {
                    Ok(_) => (),
                    Err(e) => {
                        log::error!("Failed to write to CGI script stdin: {}", e);
                        return Err(format!("Failed to write to CGI script stdin: {}", e).into());
                    }
                }
            }
        }

        // Wait for the process to complete (blocking!)
        let output = match child.wait_with_output() {
            Ok(out) => out,
            Err(e) => {
                log::error!("Failed to wait for CGI script output: {}", e);
                return Err(format!("Failed to wait for CGI script output: {}", e).into());
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::error!("CGI script failed: {}", stderr);
            return Err(format!("CGI script failed: {}", stderr).into());
        }

        match self.parse_cgi_output(&output.stdout) {
            Ok(resp) => Ok(resp),
            Err(e) => {
                log::error!("Failed to parse CGI output: {}", e);
                Err(e)
            }
        }
    }

    pub fn start_nonblocking(&self, request: CgiRequest) -> Result<CgiProcess, Box<dyn std::error::Error>> {
        if !Path::new(&request.script_path).exists() {
            return Err("CGI script not found".into());
        }

        let env_vars = self.build_environment(&request);
        let mut child = Command::new(&request.script_path)
            .envs(&env_vars)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Set pipes to non-blocking
        let stdin_fd = child.stdin.as_ref().map(|s| s.as_raw_fd());
        let stdout_fd = child.stdout.as_ref().map(|s| s.as_raw_fd());
        let stderr_fd = child.stderr.as_ref().map(|s| s.as_raw_fd());
        if let Some(fd) = stdin_fd {
            unsafe { fcntl(fd, F_SETFL, O_NONBLOCK); }
        }
        if let Some(fd) = stdout_fd {
            unsafe { fcntl(fd, F_SETFL, O_NONBLOCK); }
        }
        if let Some(fd) = stderr_fd {
            unsafe { fcntl(fd, F_SETFL, O_NONBLOCK); }
        }

        Ok(CgiProcess {
            child,
            stdin_fd,
            stdout_fd,
            stderr_fd,
        })
    }

    fn build_environment(&self, request: &CgiRequest) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Standard CGI environment variables
        env.insert("REQUEST_METHOD".to_string(), request.method.clone());
        env.insert("REQUEST_URI".to_string(), request.uri.clone());
        env.insert("QUERY_STRING".to_string(), request.query_string.clone());
        env.insert("CONTENT_LENGTH".to_string(), request.body.len().to_string());
        env.insert("REMOTE_ADDR".to_string(), request.remote_addr.clone());
        env.insert("SERVER_SOFTWARE".to_string(), "webserv/1.0".to_string());
        env.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());
        env.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());

        // Add HTTP headers as environment variables
        for (name, value) in &request.headers {
            let env_name = format!("HTTP_{}", name.to_uppercase().replace('-', "_"));
            env.insert(env_name, value.clone());
        }

        // Special handling for content-type
        if let Some(content_type) = request.headers.get("content-type") {
            env.insert("CONTENT_TYPE".to_string(), content_type.clone());
        }

        env
    }

    pub fn parse_cgi_output(&self, output: &[u8]) -> Result<CgiResponse, Box<dyn std::error::Error>> {
        let output_str = String::from_utf8_lossy(output);
        
        // Find the separator between headers and body
        if let Some(separator_pos) = output_str.find("\r\n\r\n") {
            let headers_part = &output_str[..separator_pos];
            let body_part = &output_str[separator_pos + 4..];
            
            let mut headers = HashMap::new();
            let mut status = 200;
            
            for line in headers_part.lines() {
                if let Some(colon_pos) = line.find(':') {
                    let name = line[..colon_pos].trim().to_lowercase();
                    let value = line[colon_pos + 1..].trim().to_string();
                    
                    if name == "status" {
                        if let Some(space_pos) = value.find(' ') {
                            if let Ok(status_code) = value[..space_pos].parse::<u16>() {
                                status = status_code;
                            }
                        }
                    } else {
                        headers.insert(name, value);
                    }
                }
            }
            
            Ok(CgiResponse {
                status,
                headers,
                body: body_part.as_bytes().to_vec(),
            })
        } else {
            // No headers separator found, treat entire output as body
            Ok(CgiResponse {
                status: 200,
                headers: HashMap::new(),
                body: output.to_vec(),
            })
        }
    }
}

impl Default for CgiHandler {
    fn default() -> Self {
        Self::new()
    }
}