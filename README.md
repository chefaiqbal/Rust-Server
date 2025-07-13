# Webserv - HTTP Server Implementation

A high-performance HTTP/1.1 server implementation in Rust, inspired by nginx configuration syntax.

## Features

### Phase 1 (Completed)
- ✅ Multi-server configuration support
- ✅ Non-blocking I/O with epoll
- ✅ HTTP/1.1 request/response parsing
- ✅ Route-based request handling
- ✅ Basic error handling
- ✅ Configuration file parsing (nginx-style)

### Phase 2 (In Progress)
- ✅ Static file serving with MIME type detection
- 🔄 Directory listing (autoindex)
- ✅ Basic CGI script execution
- 🔄 File upload handling
- ✅ Custom error page support

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


