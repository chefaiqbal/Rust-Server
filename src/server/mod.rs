use crate::config::{Config, ServerConfig};
use crate::http::{HttpRequest, HttpResponse};
use crate::utils::epoll::EpollManager;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::{Duration, Instant};

pub struct WebServer {
    config: Config,
    listeners: Vec<TcpListener>,
    epoll: EpollManager,
    clients: HashMap<RawFd, ClientConnection>,
    server_map: HashMap<SocketAddr, usize>, // Maps socket addr to server config index
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

impl WebServer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            listeners: Vec::new(),
            epoll: EpollManager::new().expect("Failed to create epoll"),
            clients: HashMap::new(),
            server_map: HashMap::new(),
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
        let timeout = Duration::from_millis(1000);
        
        loop {
            let events = self.epoll.wait(timeout)?;
            
            for event in events {
                if self.is_listener_fd(event.fd) {
                    self.handle_new_connection(event.fd)?;
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
        // Basic request handling - will be expanded in later phases
        println!("Handling {} request for {}", request.method, request.uri);
        
        // Check if body size exceeds limit
        if request.body.len() > server_config.client_max_body_size {
            return HttpResponse::payload_too_large();
        }
        
        // Find matching route
        for route in &server_config.routes {
            if Self::matches_route(&request.uri, &route.path) {
                // Check if method is allowed
                if !route.methods.contains(&request.method.to_string()) {
                    return HttpResponse::method_not_allowed();
                }
                
                // Handle redirect
                if let Some(ref redirect) = route.redirect {
                    return HttpResponse::redirect(redirect);
                }
                
                // For now, return a simple response
                let mut response = HttpResponse::ok();
                response.set_body_string(&format!(
                    "<html><body><h1>Hello from webserv!</h1><p>Path: {}</p></body></html>",
                    request.uri
                ));
                response.set_header("content-type", "text/html");
                return response;
            }
        }
        
        HttpResponse::not_found()
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
            println!("Closing timed out connection: {}", fd);
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