# Webserv - HTTP Server Implementation

## ✅ Features Already Implemented
- [x] Serve static files (GET)
- [x] Handle POST (including file uploads via multipart/form-data)
- [x] Basic DELETE support (if present in your handler)
- [x] Serve on multiple ports (from your config)
- [x] Custom config file with server/route settings
- [x] Serve custom error pages for 404 and 500 (other errors may need checking)
- [x] Non-blocking, single-process, single-thread (from your design)
- [x] Root directory and route mapping
- [x] Directory listing (autoindex)
- [x] CGI support for at least one language (Python)
- [x] Limit client body size (configurable)
- [x] Redirection (via config)
- [x] Compatible with HTTP/1.1 and browsers

## ❗ Features/Requirements Remaining or Needing Verification & Enhancement
1. **Never Crashes / Robust Error Handling**
    - [ ] Add error handling for all edge cases (invalid requests, panics, etc.)
    - [ ] Ensure server never panics or crashes on malformed requests or internal errors
2. **Request Timeout**
    - [ ] Implement timeout for all requests (e.g., if a request takes too long, close the connection and return 408 or 504)
3. **Single epoll (or equivalent) Call Per Client/Server Communication**
    - [ ] Audit your event loop: ensure only one epoll_wait (or equivalent) per communication step
4. **All I/O Non-blocking**
    - [ ] Double-check all file, socket, and CGI I/O is non-blocking and handled via epoll (or equivalent)
5. **Chunked and Unchunked Requests**
    - [ ] Implement and test chunked transfer encoding for both incoming requests and outgoing responses (HTTP/1.1 requirement)
    - [ ] Handle unchunked (Content-Length) requests as well
6. **Proper HTTP Status Codes**
    - [ ] Make sure every error and success path sets the correct status code (e.g., 405 for method not allowed, 413 for payload too large, etc.)
7. **Handle Cookies and Sessions**
    - [ ] Implement cookie parsing and setting in responses
    - [ ] (Optional) Implement basic session management, e.g., with a session ID cookie and in-memory map
8. **Default Error Pages for All Required Codes**
    - [ ] Ensure you have custom error pages for:
        - 400, 403, 404, 405, 413, 500
    - [ ] Place them in ./www/ and reference them in your config
9. **CGI: PATH_INFO and Environment**
    - [ ] Make sure CGI scripts receive correct environment variables, especially PATH_INFO
    - [ ] CGI script should run in the correct working directory
10. **Configuration File Features**
    - [ ] All features listed in your requirements should be configurable (host, ports, error pages, client body size, root, methods, redirection, cgi, autoindex, default file, etc.)
11. **Testing**
    - [ ] Write and run exhaustive tests:
        - Static files, uploads, deletes, chunked requests, CGI, redirections, error pages, bad configs, directory listing, etc.
    - [ ] Use siege or similar to stress test for stability and memory leaks
12. **Memory Leak Testing**
    - [ ] Use tools like valgrind or asan to check for memory leaks
13. **HTTP/1.1 Compliance**
    - [ ] Ensure persistent connections (keep-alive) work as expected
    - [ ] Properly parse and respond to all HTTP/1.1 headers
14. **Documentation**
    - [ ] Document your config options, endpoints, and any limitations in your README

A high-performance HTTP/1.1 server implementation in Rust, inspired by nginx configuration syntax.

## Configuration

### Static File Serving

The server can serve static files with proper MIME type detection. Example configuration:

```nginx
server {
    listen 8080;
    root ./www;  # Base directory for static files
    
    # Serve index.html by default
    location / {
        allow_methods GET;
        index index.html;
        autoindex on;  # Enable directory listing
    }
}
```

### CGI Support

Basic CGI script execution is supported:

```nginx
location /cgi-bin {
    allow_methods GET POST;
    root ./cgi-bin;
    cgi_pass ./python3;  # Path to interpreter
}
```

### Complete Configuration Example

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

## Debugging

The server provides detailed debug logging that can be enabled using environment variables:

```bash
# Enable debug logging for all modules
RUST_LOG=debug cargo run config/webserv.conf

# Enable debug logging for specific module (e.g., static_handler)
RUST_LOG=webserv::static_handler=debug cargo run config/webserv.conf

# More verbose logging (trace level)
RUST_LOG=trace cargo run config/webserv.conf
```

### Common Debugging Scenarios

#### Static File Serving Issues
```
# Check if the file exists at the expected location
ls -l www/

# Verify file permissions
ls -la www/
```

#### CGI Script Issues
```
# Check script permissions
chmod +x cgi-bin/your_script.py

# Test CGI script directly
./cgi-bin/your_script.py

# Check environment variables
env | sort
```

## File Uploads

### Browser Upload Form

If you visit your upload route (e.g. `http://localhost:8080/upload`) in your browser, you will see a file upload form. Select a file and click Upload. After uploading, you will see a confirmation page with a link and preview (for images).

**Sample HTML form served by the server:**
```html
<form method="POST" enctype="multipart/form-data" action="/upload">
    <input type="file" name="file" required />
    <button type="submit">Upload</button>
</form>
```

### Upload with curl

You can also upload files from the command line:
```sh
curl -F "file=@yourimage.png" http://localhost:8080/upload
```

After upload, check the upload directory (as configured by `upload_store`) for your file. The server will respond with an HTML page showing a link and preview if the file is an image.

## Requirements

- Rust 1.70+
- Linux (uses epoll system calls)
- Python 3 (for CGI testing)
```


