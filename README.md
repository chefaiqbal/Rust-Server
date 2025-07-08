# Webserv - HTTP Server Implementation

A high-performance HTTP/1.1 server implementation in Rust, inspired by nginx configuration syntax.

## Features

### Phase 1 (Current)
- ✅ Multi-server configuration support
- ✅ Non-blocking I/O with epoll
- ✅ HTTP/1.1 request/response parsing
- ✅ Route-based request handling
- ✅ Basic error handling
- ✅ Configuration file parsing (nginx-style)

### Phase 2 (Planned)
- 🔄 Static file serving
- 🔄 Directory listing (autoindex)
- 🔄 CGI script execution
- 🔄 File upload handling
- 🔄 Custom error pages

## Configuration

The server uses nginx-style configuration files:

```nginx
server {
    listen 8080;
    server_name localhost;
    client_max_body_size 1M;
    
    error_page 404 /404.html;
    
    location / {
        allow_methods GET POST DELETE;
        root ./www;
        index index.html;
        autoindex on;
    }
    
    location /api {
        allow_methods GET POST;
        root ./www;
    }
    
    location /redirect {
        return 301 http://localhost:8080/;
    }
}
```

## Usage

```bash
# Build the project
cargo build

# Run with configuration file
cargo run config/webserv.conf

# Test the server
curl http://localhost:8080/
```

## Architecture

- **Event-driven**: Uses Linux epoll for efficient I/O multiplexing
- **Non-blocking**: All I/O operations are non-blocking
- **Modular**: Clean separation between HTTP parsing, server logic, and configuration
- **Memory-safe**: Written in Rust for memory safety and performance

## Testing

```bash
# Basic GET request
curl -v http://localhost:8080/

# POST request
curl -X POST -d "test data" http://localhost:8080/api

# Test redirect
curl -v http://localhost:8080/redirect
```

## Requirements

- Rust 1.70+
- Linux (uses epoll system calls)
- Python 3 (for CGI testing)
```

```bash
git add README.md
git commit -m "docs: add comprehensive project documentation

- Add feature overview and roadmap
- Include configuration examples
- Document usage and testing instructions
- Explain architecture decisions"
```

### 3. Add Core Library Structure

```bash
git add src/lib.rs src/main.rs
git commit -m "feat: add core application structure

- Create library entry point with module declarations
- Add main.rs with argument parsing and error handling
- Set up foundation for modular architecture"
```

### 4. Add HTTP Implementation

```bash
git add src/http/
git commit -m "feat: implement HTTP/1.1 parsing and response generation

- Add comprehensive HTTP request parser with method, URI, headers
- Implement HTTP response builder with status codes
- Support for query parameters, cookies, and content negotiation
- Add proper HTTP/1.1 status code definitions
- Include helper methods for common response types"
```

### 5. Add Configuration Parser

```bash
git add src/config/
git commit -m "feat: add nginx-style configuration parser

- Parse server blocks with listen, server_name, body size limits
- Support location blocks with methods, root, index, autoindex
- Handle error page mappings and redirects
- Parse size units (K, M, G) for body size limits
- Validate configuration and provide meaningful error messages"
```

### 6. Add System Utilities

```bash
git add src/utils/
git commit -m "feat: add Linux epoll wrapper for non-blocking I/O

- Implement EpollManager for efficient event handling
- Support for adding/removing file descriptors
- Handle both listener and client socket events
- Provide clean abstraction over raw epoll system calls"
```

### 7. Add Server Implementation

```bash
git add src/server/
git commit -m "feat: implement core HTTP server with event loop

- Multi-server support with different ports and configurations
- Non-blocking accept() and I/O operations
- Connection state management and timeout handling
- Request processing pipeline with route matching
- Response buffering and streaming
- Graceful connection cleanup and resource management"
```

### 8. Add CGI Foundation

```bash
git add src/cgi/
git commit -m "feat: add CGI handler foundation for Phase 2

- Define CGI request/response structures
- Environment variable building for CGI scripts
- Process spawning and communication framework
- Output parsing for headers and body separation
- Timeout handling for CGI execution"
```

### 9. Add Test Configuration and Files

```bash
git add config/ www/ cgi-bin/
git commit -m "feat: add sample configuration and test files

- Sample webserv.conf with multiple server blocks
- Test HTML files for basic serving
- CGI test script for dynamic content
- Error page templates
- Complete testing setup for development"
```

### 10. Final Integration Commit

```bash
git add .
git commit -m "feat: complete Phase 1 implementation

- Fully functional HTTP/1.1 server
- Multi-server configuration support
- Non-blocking I/O with epoll
- Route-based request handling
- Ready for Phase 2 enhancements (file serving, CGI, uploads)"
```

## Push to GitHub

Now create your GitHub repository and push:

```bash
# Add your GitHub repository as remote
git remote add origin https://github.com/yourusername/localhost.git

# Push to GitHub
git push -u origin main
```

## Repository Description

For your GitHub repository, use this description:
```
High-performance HTTP/1.1 server in Rust with nginx-style configuration, non-blocking I/O, and modular architecture