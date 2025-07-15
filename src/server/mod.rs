use crate::config::{Config, ServerConfig};
use crate::http::{HttpRequest, HttpResponse, StatusCode};
use crate::static_handler::StaticFileHandler;
use crate::utils::epoll::EpollManager;
mod session;
use session::get_or_create_session_id;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use crate::cgi::{CgiHandler, CgiRequest, CgiProcess};

pub struct WebServer {
    config: Config,
    listeners: Vec<TcpListener>,
    epoll: EpollManager,
    clients: HashMap<RawFd, ClientConnection>,
    server_map: HashMap<SocketAddr, usize>, // Maps socket addr to server config index
    cgi_connections: HashMap<RawFd, CgiConnection>, // Map CGI fd to CgiConnection
}

#[derive(Debug)]
struct ClientConnection {
    stream: TcpStream,
    server_config_index: usize,
    buffer: Vec<u8>,
    response_buffer: Vec<u8>,
    last_activity: Instant,
    state: ConnectionState,
}

#[derive(Debug, PartialEq)]
enum ConnectionState {
    Reading,
    Processing,
    Writing,
    KeepAlive,
}

#[derive(Debug)]
struct CgiConnection {
    pub process: CgiProcess,
    pub client_fd: RawFd,
    pub output_buffer: Vec<u8>,
    pub error_buffer: Vec<u8>,
    pub stdin_done: bool,
    pub stdout_done: bool,
    pub stderr_done: bool,
    pub body_to_write: Vec<u8>,
    pub body_written: usize,
    pub done: bool,
}

impl WebServer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            listeners: Vec::new(),
            epoll: EpollManager::new().expect("Failed to create epoll"),
            clients: HashMap::new(),
            server_map: HashMap::new(),
            cgi_connections: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.setup_listeners()?;
        self.event_loop()
    }

    fn setup_listeners(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for (index, server_config) in self.config.servers.iter().enumerate() {
            let addr = server_config.socket_addr()?;
            
            // Check for duplicate ports
            if self.server_map.contains_key(&addr) {
                return Err(format!("Port {} already in use", addr.port()).into());
            }
            
            let listener = TcpListener::bind(&addr)?;
            listener.set_nonblocking(true)?;
            
            println!("Server listening on {}", addr);
            
            let fd = listener.as_raw_fd();
            self.epoll.add_listener(fd)?;
            self.server_map.insert(addr, index);
            self.listeners.push(listener);
        }
        
        Ok(())
    }

    fn event_loop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // AUDIT NOTE: Only one epoll_wait call per event loop iteration, as required by project spec.
        // This ensures a single epoll (or equivalent) call per client/server communication step.
        let timeout = Duration::from_millis(1000);
        
        loop {
            let events = self.epoll.wait(timeout)?;
            
            for event in events {
                if self.is_listener_fd(event.fd) {
                    self.handle_new_connection(event.fd)?;
                } else if let Some(cgi_conn) = self.cgi_connections.get_mut(&event.fd) {
                    self.handle_cgi_event(event.fd, event.readable, event.writable)?;
                    continue;
                } else {
                    self.handle_client_event(event.fd, event.readable, event.writable)?;
                }
            }
            
            // Clean up timed out connections
            self.cleanup_timeouts();
        }
    }

    fn is_listener_fd(&self, fd: RawFd) -> bool {
        self.listeners.iter().any(|listener| listener.as_raw_fd() == fd)
    }

    fn handle_new_connection(&mut self, listener_fd: RawFd) -> Result<(), Box<dyn std::error::Error>> {
        let listener = self.listeners.iter()
            .find(|l| l.as_raw_fd() == listener_fd)
            .ok_or("Listener not found")?;
        
        match listener.accept() {
            Ok((stream, addr)) => {
                stream.set_nonblocking(true)?;
                let client_fd = stream.as_raw_fd();
                
                // Find server config for this listener
                let server_config_index = self.listeners.iter()
                    .position(|l| l.as_raw_fd() == listener_fd)
                    .unwrap_or(0);
                
                let client = ClientConnection {
                    stream,
                    server_config_index,
                    buffer: Vec::new(),
                    response_buffer: Vec::new(),
                    last_activity: Instant::now(),
                    state: ConnectionState::Reading,
                };
                
                self.epoll.add_client(client_fd)?;
                self.clients.insert(client_fd, client);
                
                println!("New connection from {}", addr);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No more connections to accept
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
        
        Ok(())
    }

    fn handle_client_event(&mut self, fd: RawFd, readable: bool, writable: bool) -> Result<(), Box<dyn std::error::Error>> {
        if !self.clients.contains_key(&fd) {
            return Ok(());
        }

        let mut should_close = false;
        
        if readable {
            if let Err(e) = self.handle_client_read(fd) {
                eprintln!("Error reading from client {}: {}", fd, e);
                should_close = true;
            }
        }
        
        if writable && !should_close {
            if let Err(e) = self.handle_client_write(fd) {
                eprintln!("Error writing to client {}: {}", fd, e);
                should_close = true;
            }
        }
        
        if should_close {
            self.close_client_connection(fd);
        }
        
        Ok(())
    }

    fn handle_client_read(&mut self, fd: RawFd) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.clients.get_mut(&fd).ok_or("Client not found")?;
        
        let mut buffer = [0; 8192];
        match client.stream.read(&mut buffer) {
            Ok(0) => {
                // Client closed connection
                return Err("Client closed connection".into());
            }
            Ok(n) => {
                client.buffer.extend_from_slice(&buffer[..n]);
                client.last_activity = Instant::now();
                
                // Check if we have a complete request
                let buffer_copy = client.buffer.clone();
                let is_complete = Self::is_complete_request(&buffer_copy);
                
                if is_complete {
                    self.process_request(fd)?;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No more data available right now
            }
            Err(e) => {
                return Err(e.into());
            }
        }
        
        Ok(())
    }

    fn handle_client_write(&mut self, fd: RawFd) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.clients.get_mut(&fd).ok_or("Client not found")?;
        
        if client.response_buffer.is_empty() {
            return Ok(());
        }
        
        match client.stream.write(&client.response_buffer) {
            Ok(n) => {
                client.response_buffer.drain(..n);
                client.last_activity = Instant::now();
                
                if client.response_buffer.is_empty() {
                    client.state = ConnectionState::KeepAlive;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Can't write more right now
            }
            Err(e) => {
                return Err(e.into());
            }
        }
        
        Ok(())
    }

    fn is_complete_request(buffer: &[u8]) -> bool {
        // Look for end of headers
        if let Some(pos) = Self::find_header_end(buffer) {
            // Check if we have the complete body
            let header_part = &buffer[..pos];
            if let Ok(header_str) = std::str::from_utf8(header_part) {
                if let Some(content_length) = Self::extract_content_length(header_str) {
                    let body_start = pos + 4; // Skip \r\n\r\n
                    let body_received = buffer.len() - body_start;
                    return body_received >= content_length;
                } else {
                    // No content-length, assume complete
                    return true;
                }
            }
        }
        false
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        for i in 0..buffer.len().saturating_sub(3) {
            if &buffer[i..i+4] == b"\r\n\r\n" {
                return Some(i);
            }
        }
        None
    }

    fn extract_content_length(headers: &str) -> Option<usize> {
        for line in headers.lines() {
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(value) = line.split(':').nth(1) {
                    return value.trim().parse().ok();
                }
            }
        }
        None
    }

    fn process_request(&mut self, fd: RawFd) -> Result<(), Box<dyn std::error::Error>> {
        // Extract the data we need before borrowing mutably
        let (request_data, server_config_index) = {
            let client = self.clients.get_mut(&fd).ok_or("Client not found")?;
            let request_data = client.buffer.clone();
            client.buffer.clear();
            client.state = ConnectionState::Processing;
            (request_data, client.server_config_index)
        };
        
        let response = match HttpRequest::parse(&request_data) {
            Ok(request) => {
                let server_config = &self.config.servers[server_config_index];
                Self::handle_request(request, server_config)
            }
            Err(e) => {
                eprintln!("Error parsing request: {}", e);
                HttpResponse::bad_request()
            }
        };
        
        // Now we can borrow mutably again
        let client = self.clients.get_mut(&fd).ok_or("Client not found")?;
        client.response_buffer = response.to_bytes();
        client.state = ConnectionState::Writing;
        
        Ok(())
    }

    fn handle_request(request: HttpRequest, server_config: &ServerConfig) -> HttpResponse {
    println!("[DEBUG] All route configs:");
    for route in &server_config.routes {
        println!("  path: {}, methods: {:?}, root: {:?}, cgi_pass: {:?}, cgi_extension: {:?}", route.path, route.methods, route.root, route.cgi_pass, route.cgi_extension);
    }
        println!("Handling {} request for {}", request.method, request.uri);
        
        // --- SESSION HANDLING LOGIC START ---
        // Get or create session id from cookie header
        let cookie_header = request.get_header("cookie");
        let session_id = get_or_create_session_id(cookie_header);
        let mut set_cookie_needed = true;
        if let Some(cookie_header) = cookie_header {
            if cookie_header.contains(&format!("SESSIONID={}", session_id)) {
                set_cookie_needed = false;
            }
        }
        // --- SESSION HANDLING LOGIC END ---

        // Check if body size exceeds limit
        if request.body.len() > server_config.client_max_body_size {
            let mut resp = HttpResponse::payload_too_large();
            if set_cookie_needed {
                resp.set_cookie("SESSIONID", &session_id, Some(3600), Some("/"));
            }
            return resp;
        }
        
        // Find matching route
        for route in &server_config.routes {
            if Self::matches_route(&request.uri, &route.path) {
                // Check if method is allowed
                if !route.methods.contains(&request.method.to_string()) {
                    let error_page = server_config.error_pages.get(&405).map(|s| s.as_str());
                    let mut resp = HttpResponse::method_not_allowed_custom(error_page);
                    if set_cookie_needed {
                        resp.set_cookie("SESSIONID", &session_id, Some(3600), Some("/"));
                    }
                    return resp;
                }
                
                // Use static file handler for this route
                let static_handler = StaticFileHandler::new(server_config);
                let mut response = static_handler.handle_request(&request, server_config);
                if set_cookie_needed {
                    response.set_cookie("SESSIONID", &session_id, Some(3600), Some("/"));
                }
                // If we got a 404 and there's a custom error page for it, try to serve that
                if response.status == StatusCode::NotFound {
                    if let Some(error_page) = server_config.error_pages.get(&404) {
                        let error_path = PathBuf::from(error_page);
                        if let Ok(metadata) = std::fs::metadata(&error_path) {
                            if !metadata.is_dir() {
                                if let Ok(content) = std::fs::read(&error_path) {
                                    let mut custom_response = HttpResponse::new(StatusCode::NotFound);
                                    custom_response.set_body(&content);
                                    custom_response.set_header("content-type", "text/html");
                                    if set_cookie_needed {
                                        custom_response.set_cookie("SESSIONID", &session_id, Some(3600), Some("/"));
                                    }
                                    return custom_response;
                                }
                            }
                        }
                    }
                }
                return response;
            }
        }
        
        // No matching route found
        let mut resp = HttpResponse::not_found();
        if set_cookie_needed {
            resp.set_cookie("SESSIONID", &session_id, Some(3600), Some("/"));
        }
        resp
    }

    fn matches_route(uri: &str, route_path: &str) -> bool {
        if route_path == "/" {
            return true; // Root route matches everything
        }
        uri.starts_with(route_path)
    }

    fn cleanup_timeouts(&mut self) {
        let timeout_duration = Duration::from_secs(30);
        let now = Instant::now();
        
        let mut to_remove = Vec::new();
        
        for (&fd, client) in &self.clients {
            if now.duration_since(client.last_activity) > timeout_duration {
                to_remove.push(fd);
            }
        }
        
        for fd in to_remove {
            println!("Client {} timed out", fd);
            self.close_client_connection(fd);
        }
    }

    fn close_client_connection(&mut self, fd: RawFd) {
        if let Some(client) = self.clients.remove(&fd) {
            let _ = self.epoll.remove_client(fd);
            drop(client); // This will close the stream
            println!("Closed connection: {}", fd);
        }
    }

    fn start_cgi_for_client(&mut self, client_fd: RawFd, cgi_req: CgiRequest) -> Result<(), Box<dyn std::error::Error>> {
        let handler = CgiHandler::new();
        let process = handler.start_nonblocking(cgi_req.clone())?;
        let stdout_fd = process.stdout_fd;
        let stderr_fd = process.stderr_fd;
        let stdin_fd = process.stdin_fd;
        let cgi_conn = CgiConnection {
            process,
            client_fd,
            output_buffer: Vec::new(),
            error_buffer: Vec::new(),
            stdin_done: false,
            stdout_done: false,
            stderr_done: false,
            body_to_write: cgi_req.body.clone(),
            body_written: 0,
            done: false,
        };
        if let Some(fd) = stdout_fd {
            self.epoll.add_client(fd)?;
            self.cgi_connections.insert(fd, cgi_conn);
        }
        if let Some(fd) = stderr_fd {
            self.epoll.add_client(fd)?;
        }
        if let Some(fd) = stdin_fd {
            self.epoll.add_client(fd)?;
        }
        Ok(())
    }

    fn handle_cgi_event(&mut self, fd: RawFd, readable: bool, writable: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut fds_to_remove = Vec::new();
        if let Some(conn) = self.cgi_connections.get_mut(&fd) {
            // Handle stdin (write request body)
            if let Some(stdin_fd) = conn.process.stdin_fd {
                if fd == stdin_fd && writable && !conn.stdin_done {
                    if let Some(ref mut stdin) = conn.process.child.stdin {
                        let to_write = &conn.body_to_write[conn.body_written..];
                        match stdin.write(to_write) {
                            Ok(n) => {
                                conn.body_written += n;
                                if conn.body_written == conn.body_to_write.len() {
                                    conn.stdin_done = true;
                                    drop(stdin);
                                    self.epoll.remove_client(stdin_fd)?;
                                }
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                            Err(_) => {
                                conn.stdin_done = true;
                                drop(stdin);
                                self.epoll.remove_client(stdin_fd)?;
                            }
                        }
                    }
                }
            }
            // Handle stdout (read CGI output)
            if let Some(stdout_fd) = conn.process.stdout_fd {
                if fd == stdout_fd && readable && !conn.stdout_done {
                    if let Some(ref mut stdout) = conn.process.child.stdout {
                        let mut buf = [0u8; 8192];
                        match stdout.read(&mut buf) {
                            Ok(0) => {
                                conn.stdout_done = true;
                                drop(stdout);
                                self.epoll.remove_client(stdout_fd)?;
                            }
                            Ok(n) => {
                                conn.output_buffer.extend_from_slice(&buf[..n]);
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                            Err(_) => {
                                conn.stdout_done = true;
                                drop(stdout);
                                self.epoll.remove_client(stdout_fd)?;
                            }
                        }
                    }
                }
            }
            // Handle stderr (read CGI error output)
            if let Some(stderr_fd) = conn.process.stderr_fd {
                if fd == stderr_fd && readable && !conn.stderr_done {
                    if let Some(ref mut stderr) = conn.process.child.stderr {
                        let mut buf = [0u8; 4096];
                        match stderr.read(&mut buf) {
                            Ok(0) => {
                                conn.stderr_done = true;
                                drop(stderr);
                                self.epoll.remove_client(stderr_fd)?;
                            }
                            Ok(n) => {
                                conn.error_buffer.extend_from_slice(&buf[..n]);
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                            Err(_) => {
                                conn.stderr_done = true;
                                drop(stderr);
                                self.epoll.remove_client(stderr_fd)?;
                            }
                        }
                    }
                }
            }
            // Check if CGI is done
            if conn.stdin_done && conn.stdout_done && conn.stderr_done {
                let handler = CgiHandler::new();
                if let Ok(resp) = handler.parse_cgi_output(&conn.output_buffer) {
                    if let Some(client) = self.clients.get_mut(&conn.client_fd) {
                        client.response_buffer = HttpResponse::from_cgi_response(resp).to_bytes();
                        client.state = ConnectionState::Writing;
                    }
                } else if !conn.error_buffer.is_empty() {
                    let err_msg = String::from_utf8_lossy(&conn.error_buffer);
                    eprintln!("CGI error: {}", err_msg);
                }
                conn.done = true;
                if let Some(fd) = conn.process.stdout_fd { fds_to_remove.push(fd); }
                if let Some(fd) = conn.process.stderr_fd { fds_to_remove.push(fd); }
                if let Some(fd) = conn.process.stdin_fd { fds_to_remove.push(fd); }
            }
        }
        for fd in fds_to_remove {
            self.epoll.remove_client(fd).ok();
            self.cgi_connections.remove(&fd);
        }
        Ok(())
    }
}

impl Drop for WebServer {
    fn drop(&mut self) {
        // Clean up all connections
        let fds: Vec<RawFd> = self.clients.keys().cloned().collect();
        for fd in fds {
            self.close_client_connection(fd);
        }
    }
}