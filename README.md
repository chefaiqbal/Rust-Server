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
- [x] Single epoll (or equivalent) call per client/server communication (see Architecture section)

## ❗ Features/Requirements Remaining or Needing Verification & Enhancement
1. **Never Crashes / Robust Error Handling**
    - [ ] Add error handling for all edge cases (invalid requests, panics, etc.)
    - [ ] Ensure server never panics or crashes on malformed requests or internal errors
2. **Request Timeout** ✅
    - [x] Implement timeout for all requests (e.g., if a request takes too long, close the connection and return 408 or 504)
    - [x] Configurable timeout duration per server
    - [x] Proper HTTP status codes (408 Request Timeout and 504 Gateway Timeout)
    - [x] Clean connection closure on timeout
3. **Single epoll (or equivalent) Call Per Client/Server Communication** ✅
    - [x] Only one epoll_wait (or equivalent) call per event loop iteration (see Architecture section)
4. **All I/O Non-blocking**
    - [x] Double-check all file, socket, and CGI I/O is non-blocking and handled via epoll (or equivalent)
5. **Chunked and Unchunked Requests**
    - [ ] Implement and test chunked transfer encoding for both incoming requests and outgoing responses (HTTP/1.1 requirement)
    - [ ] Handle unchunked (Content-Length) requests as well
6. **Proper HTTP Status Codes**
    - [x] Make sure every error and success path sets the correct status code (e.g., 405 for method not allowed, 413 for payload too large, etc.)
7. **Handle Cookies and Sessions**
    - [x] Implement cookie parsing and setting in responses
    - [x] Implement basic session management, with a session ID cookie and in-memory map
8. **Default Error Pages for All Required Codes**
    - [x] Ensure you have custom error pages for:
        - 400, 403, 404, 405, 413, 500
    - [x] Place them in ./www/ and reference them in your config
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

## Cookie and Session Handling

The server supports both cookie parsing/setting and minimal session management:

- **Cookie Parsing:** Incoming requests are parsed for the `Cookie` header; cookies are available in the request handler.
- **Cookie Setting:** Use the `set_cookie` method in `HttpResponse` to set cookies in responses.
- **Session Management:**
    - On each request, the server checks for a `SESSIONID` cookie.
    - If missing, a new random session ID is generated and stored in a global in-memory session map, and sent as a `Set-Cookie` header.
    - If present, the session ID is reused.
    - This demonstrates a minimal session system for educational purposes.

### Testing Cookies and Sessions

You can test cookie and session handling with `curl`:

```sh
# See Set-Cookie header from the server
curl -i http://localhost:8080/

# Use the SESSIONID from above to simulate a returning client
curl -i --cookie "SESSIONID=YOUR_SESSION_ID" http://localhost:8080/
```

Or use your browser's developer tools to inspect cookies.

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
- **Single epoll_wait per event loop**: The event loop is designed to call epoll_wait (or equivalent) only once per iteration, as required by the project specification. All client/server communication steps (accept, read, write) are handled in response to the events returned by this single call. This ensures compliance with the audit and project requirements for efficient I/O multiplexing.

## Epoll Usage and Event Loop Implementation

The server's event loop is implemented in `src/server/mod.rs` and uses a custom epoll wrapper in `src/utils/epoll.rs`. Only one call to `epoll_wait` is made per event loop iteration, as required by the project specification. All client and server socket I/O (accept, read, write) is handled in response to the events returned by this single call. No blocking I/O is performed outside of epoll events. This design ensures:

- Efficient, scalable handling of many connections in a single thread
- Compliance with the audit requirement for a single epoll (or equivalent) call per communication step
- All reads and writes are performed only when epoll signals readiness

**Relevant code locations:**
- Event loop: [`src/server/mod.rs`](src/server/mod.rs), see `fn event_loop()`
- Epoll wrapper: [`src/utils/epoll.rs`](src/utils/epoll.rs)

This approach is similar to how high-performance servers like nginx operate, and is required for full marks in the audit.

## Request Timeout Configuration

The server implements configurable request timeouts to handle slow clients and long-running requests. By default, the timeout is set to 30 seconds.

### Configuration

In your server configuration, you can set the `request_timeout_secs` parameter:

```toml
[server]
listen = 8080
server_name = "localhost"
request_timeout_secs = 30  # Timeout in seconds
```

### Behavior

- **408 Request Timeout**: Sent when the client takes too long to send the complete request.
- **504 Gateway Timeout**: Sent when the server takes too long to process a request.
- The connection is automatically closed after sending the timeout response.

### Testing Timeouts

You can test the timeout behavior using the provided test script:

```bash
./test_timeout.sh
```

Or manually with `curl`:

```bash
# Test request timeout (408)
curl -v -X POST http://localhost:8080/slow_request -d "data=test" -H "Content-Type: application/x-www-form-urlencoded" -m 10
```

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


