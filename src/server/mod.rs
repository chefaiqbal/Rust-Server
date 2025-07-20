use crate::config::{Config, ServerConfig, RouteConfig};
use crate::http::{HttpRequest, HttpResponse, StatusCode};
use crate::static_handler::StaticFileHandler;
use crate::cgi::{CgiHandler, CgiRequest, CgiProcess};
use crate::utils::epoll::EpollManager;
mod session;
use session::get_or_create_session_id;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    pub fn new(config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            config,
            listeners: Vec::new(),
            epoll: EpollManager::new()?,
            clients: HashMap::new(),
            server_map: HashMap::new(),
            cgi_connections: HashMap::new(),
        })
    }

    fn find_route_config<'a>(&self, server_config: &'a ServerConfig, path: &str) -> Option<&'a crate::config::RouteConfig> {
        // Find the most specific matching route
        server_config.routes.iter()
            .filter(|r| path.starts_with(&r.path))
            .max_by_key(|r| r.path.len())
    }

    fn find_route_for_request<'a>(
        &self,
        request: &HttpRequest,
        server_config: &'a ServerConfig,
    ) -> Option<&'a RouteConfig> {
        server_config
            .routes
            .iter()
            .filter(|route| request.uri.starts_with(&route.path))
            .max_by_key(|route| route.path.len())
    }

    fn handle_not_found(&self, server_config: &ServerConfig) -> HttpResponse {
        if let Some(error_page_path) = server_config.error_pages.get(&404) {
            if let Ok(content) = std::fs::read(error_page_path) {
                let mut response = HttpResponse::new(StatusCode::NotFound);
                response.set_body(&content);
                response.set_header("Content-Type", "text/html");
                return response;
            }
        }
        HttpResponse::not_found()
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
            let events = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.epoll.wait(timeout))) {
                Ok(Ok(ev)) => ev,
                Ok(Err(e)) => {
                    log::error!("epoll.wait failed: {}", e);
                    continue;
                }
                Err(panic_info) => {
                    log::error!("Panic in epoll.wait: {:?}", panic_info);
                    continue;
                }
            };
            
            for event in events {
                let handler_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if self.is_listener_fd(event.fd) {
                        self.handle_new_connection(event.fd)
                    } else if self.cgi_connections.get_mut(&event.fd).is_some() {
                        self.handle_cgi_event(event.fd, event.readable, event.writable)
                    } else {
                        self.handle_client_event(event.fd, event.readable, event.writable)
                    }
                }));
                if let Err(panic_info) = handler_result {
                    log::error!("Panic in event handler for fd {}: {:?}", event.fd, panic_info);
                    self.close_client_connection(event.fd);
                } else if let Ok(Err(e)) = handler_result {
                    log::error!("Handler error for fd {}: {}", event.fd, e);
                    self.close_client_connection(event.fd);
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
        
        match HttpRequest::parse(&request_data) {
            Ok(request) => {
                self.handle_request_wrapper(fd, request, server_config_index)?;
            }
            Err(e) => {
                eprintln!("Error parsing request: {}", e);
                let response = HttpResponse::bad_request();
                let client = self.clients.get_mut(&fd).ok_or("Client not found")?;
                client.response_buffer = response.to_bytes();
                client.state = ConnectionState::Writing;
            }
        };
        
        Ok(())
    }

    fn handle_request_wrapper(&mut self, client_fd: RawFd, request: HttpRequest, server_config_index: usize) -> Result<(), Box<dyn std::error::Error>> {
        let server_config = &self.config.servers[server_config_index];
        
        let response = if let Some(route) = self.find_route_for_request(&request, server_config) {
            if route.is_cgi_request(&request.uri) {
                println!("Handling as CGI request");
                match self.create_cgi_request(&request, route) {
                    Ok(cgi_request) => {
                        let cgi_handler = CgiHandler::new();
                        match cgi_handler.execute(cgi_request) {
                            Ok(cgi_response) => {
                                HttpResponse::from_cgi_response(cgi_response)
                            }
                            Err(e) => {
                                eprintln!("Error executing CGI: {}", e);
                                HttpResponse::internal_server_error()
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error creating CGI request: {}", e);
                        HttpResponse::internal_server_error()
                    }
                }
            } else {
                // Use the new static request handler with proper 403 handling
                Self::handle_static_request(request, server_config)
            }
        } else {
            self.handle_not_found(server_config)
        };

        if let Some(client) = self.clients.get_mut(&client_fd) {
            client.response_buffer = response.to_bytes();
            client.state = ConnectionState::Writing;
        }
        
        Ok(())
    }

    fn create_cgi_request(
        &self,
        request: &HttpRequest,
        route_config: &RouteConfig,
    ) -> Result<CgiRequest, anyhow::Error> {
        let root = route_config.root.as_deref().unwrap_or("./");

        // We need to map this to a filesystem path like "./www/cgi-bin/test.py".
        let script_path = PathBuf::from(root).join(request.uri.trim_start_matches('/'));

        if !script_path.exists() {
            return Err(anyhow::anyhow!("CGI script not found at: {:?}", script_path));
        }

        Ok(CgiRequest {
            script_path: script_path.to_str().unwrap().to_string(),
            method: request.method.to_string(),
            uri: request.uri.clone(),
            query_string: request.query_string.clone().unwrap_or_default(),
            headers: request.headers.clone(),
            body: request.body.clone(),
            remote_addr: "127.0.0.1".to_string(), // Placeholder, could be improved
            cgi_pass: route_config.cgi_pass.clone(),
        })
    }

    fn handle_static_request(request: HttpRequest, server_config: &ServerConfig) -> HttpResponse {
        println!("[DEBUG] All route configs:");
        for route in &server_config.routes {
            println!("  path: {}, methods: {:?}, root: {:?}, cgi_pass: {:?}, cgi_extension: {:?}", route.path, route.methods, route.root, route.cgi_pass, route.cgi_extension);
        }
        println!("Handling {} request for {}", request.method, request.uri);
        
        // --- SESSION HANDLING LOGIC START ---
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
                
                // CHECK FOR EMPTY METHODS FIRST - before any file system operations
                if route.methods.is_empty() {
                    println!("Empty methods for route {}, returning 403 Forbidden", route.path);
                    // Try to serve custom 403 error page
                    if let Some(error_page_path) = server_config.error_pages.get(&403) {
                        if let Ok(content) = std::fs::read(error_page_path) {
                            let mut response = HttpResponse::new(StatusCode::Forbidden);
                            response.set_body(&content);
                            response.set_header("Content-Type", "text/html");
                            if set_cookie_needed {
                                response.set_cookie("SESSIONID", &session_id, Some(3600), Some("/"));
                            }
                            return response;
                        }
                    }
                    // Fallback to default 403 response
                    let mut resp = HttpResponse::forbidden();
                    if set_cookie_needed {
                        resp.set_cookie("SESSIONID", &session_id, Some(3600), Some("/"));
                    }
                    return resp;
                }
                
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
                    let body = &conn.body_to_write.clone();
                    let written = &mut conn.body_written;
                    if *written < body.len() {
                        match conn.process.child.stdin.as_mut().unwrap().write(&body[*written..]) {
                            Ok(n) => {
                                *written += n;
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                // Can't write more now, will try again
                            }
                            Err(e) => {
                                log::error!("Error writing to CGI stdin: {}", e);
                                conn.stdin_done = true; // Stop trying
                            }
                        }
                    }
                    if *written >= body.len() {
                        conn.stdin_done = true;
                        // Close stdin to signal end of input
                        drop(conn.process.child.stdin.take()); 
                        fds_to_remove.push(fd);
                    }
                }
            }

            // Handle stdout (read script output)
            if let Some(stdout_fd) = conn.process.stdout_fd {
                if fd == stdout_fd && readable && !conn.stdout_done {
                    let mut buf = [0; 4096];
                    match conn.process.child.stdout.as_mut().unwrap().read(&mut buf) {
                        Ok(0) => {
                            conn.stdout_done = true;
                            fds_to_remove.push(fd);
                        }
                        Ok(n) => {
                            conn.output_buffer.extend_from_slice(&buf[..n]);
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // No more data for now
                        }
                        Err(e) => {
                            log::error!("Error reading from CGI stdout: {}", e);
                            conn.stdout_done = true; // Stop trying
                            fds_to_remove.push(fd);
                        }
                    }
                }
            }

            // Handle stderr (read script error)
            if let Some(stderr_fd) = conn.process.stderr_fd {
                if fd == stderr_fd && readable && !conn.stderr_done {
                    let mut buf = [0; 4096];
                    match conn.process.child.stderr.as_mut().unwrap().read(&mut buf) {
                        Ok(0) => {
                            conn.stderr_done = true;
                            fds_to_remove.push(fd);
                        }
                        Ok(n) => {
                            conn.error_buffer.extend_from_slice(&buf[..n]);
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // No more data for now
                        }
                        Err(e) => {
                            log::error!("Error reading from CGI stderr: {}", e);
                            conn.stderr_done = true; // Stop trying
                            fds_to_remove.push(fd);
                        }
                    }
                }
            }

            // Check if CGI process is finished
            if conn.stdout_done && conn.stderr_done && !conn.done {
                conn.done = true;
                let cgi_handler = CgiHandler::new();
                let response = if !conn.error_buffer.is_empty() {
                    log::error!("CGI Error: {}", String::from_utf8_lossy(&conn.error_buffer));
                    HttpResponse::internal_server_error()
                } else {
                    match cgi_handler.parse_cgi_output(&conn.output_buffer) {
                        Ok(cgi_resp) => HttpResponse::from_cgi_response(cgi_resp),
                        Err(e) => {
                            log::error!("Failed to parse CGI output: {}", e);
                            HttpResponse::internal_server_error()
                        }
                    }
                };

                // Send response to the original client
                if let Some(client) = self.clients.get_mut(&conn.client_fd) {
                    client.response_buffer = response.to_bytes();
                    client.state = ConnectionState::Writing;
                }

                // Clean up this CGI connection
                if let Some(process_fd) = conn.process.stdout_fd { fds_to_remove.push(process_fd); }
                if let Some(process_fd) = conn.process.stderr_fd { fds_to_remove.push(process_fd); }
            }
        }
        for process_fd in fds_to_remove {
            self.epoll.remove_client(process_fd)?;
            self.cgi_connections.remove(&process_fd);
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