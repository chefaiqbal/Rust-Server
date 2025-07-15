# Audit Answers for Localhost HTTP Server

---

## Functional

### How does an HTTP server work?
An HTTP server listens for incoming TCP connections on specified ports, accepts connections from clients (like browsers), reads HTTP requests, processes them (e.g., serving static files, handling routes, invoking CGI scripts), and sends back HTTP responses.
- **Implementation:** See `src/server/mod.rs` (`WebServer` struct, `setup_listeners`, `event_loop`).

### Which function was used for I/O Multiplexing and how does it work?
The server uses **epoll** for I/O multiplexing, via the custom `EpollManager` (see `src/utils/epoll.rs` if present, imported in `src/server/mod.rs`). The main event loop (`WebServer::event_loop`) calls `self.epoll.wait(timeout)` once per iteration to wait for events on all sockets, allowing efficient handling of many connections with a single thread.

### Is the server using only one select (or equivalent) to read the client requests and write answers?
**Yes.** Only one `epoll_wait` is called per event loop iteration (`WebServer::event_loop`).

### Why is it important to use only one select and how was it achieved?
Using a single `select`/`epoll` per loop ensures efficient, non-blocking, scalable handling of many connections without busy-waiting or unnecessary system calls. Achieved by centralizing all socket readiness checks in the `epoll.wait()` call in the event loop.

### Read the code that goes from the select (or equivalent) to the read and write of a client, is there only one read or write per client per select (or equivalent)?
**Yes.** After `epoll_wait`, for each ready file descriptor, `handle_client_read` or `handle_client_write` is called. Each function attempts a single read or write per event, handling `WouldBlock` properly. See `WebServer::handle_client_read` and `WebServer::handle_client_write` in `src/server/mod.rs`.

### Are the return values for I/O functions checked properly?
**Yes.** All `read` and `write` calls check for errors and handle `WouldBlock` and other errors explicitly. On error, the connection is closed and cleaned up.

### If an error is returned by the previous functions on a socket, is the client removed?
**Yes.** On error, `should_close` is set and `close_client_connection` is called, removing the client from the map and epoll.
- Files: `src/server/mod.rs`

### Is writing and reading ALWAYS done through a select (or equivalent)?
**Yes.** All reads and writes are triggered only when epoll signals readiness, ensuring non-blocking I/O.
- Files: `src/server/mod.rs`, `src/utils/epoll.rs`

---

## Configuration file

### Setup a single server with a single port
- Files: `config/webserv.conf`, `src/config/mod.rs`

### Setup multiple servers with different ports
- Files: `config/webserv.conf`, `src/config/mod.rs`

### Setup multiple servers with different hostnames
- Files: `config/webserv.conf`, `src/config/mod.rs`

### Setup custom error pages
- Files: `config/webserv.conf`, `src/config/mod.rs`, `src/server/mod.rs`, `src/static_handler.rs`, `src/http/response.rs`

### Limit the client body
- Files: `config/webserv.conf`, `src/config/mod.rs`, `src/server/mod.rs`

### Setup routes and ensure they are taken into account
- Files: `config/webserv.conf`, `src/config/mod.rs`, `src/server/mod.rs`, `src/static_handler.rs`

### Setup a default file in case the path is a directory
- Files: `config/webserv.conf`, `src/static_handler.rs`

### Setup a list of accepted methods for a route
Supported: Each route specifies allowed HTTP methods; requests with disallowed methods return proper error codes.

---

## Methods and cookies

### Are the GET requests working properly?
Supported and handled in `handle_request` (`src/server/mod.rs`).

### Are the POST requests working properly?
Supported, including file uploads and CGI.

### Are the DELETE requests working properly?
Supported if enabled for the route.

### Test a WRONG request, is the server still working properly?
Malformed or unsupported requests return 400/405/404 as appropriate.

### Upload some files to the server and get them back to test they were not corrupted
File upload is supported (see POST handling and upload directory creation in `main.rs`).

### A working session and cookies system is present on the server?
Yes. Sessions are managed in `src/server/session.rs` with `SESSIONID` cookies set via `HttpResponse::set_cookie`.

---

## Interaction with the browser

### Is the browser connecting with the server with no issues?
Yes, standard HTTP/1.1 is supported.

### Are the request and response headers correct?
Yes, headers are parsed and set according to HTTP spec.

### Try a wrong URL on the server, is it handled properly?
Returns 404 with a custom error page if configured.

### Try to list a directory, is it handled properly?
Directory listing or default file serving is handled by the static handler.

### Try a redirected URL, is it handled properly?
Redirects can be configured per route.

### Check the implemented CGI, does it work properly with chunked and unchunked data?
Yes, CGI is supported (see `src/cgi/mod.rs`), and chunked transfer encoding is handled in request parsing.

---

## Port issues

### Configure multiple ports and websites and ensure it is working as expected
Supported; each server block can have its own port.

### Configure the same port multiple times. The server should find the error
Yes: In `WebServer::setup_listeners`, duplicate ports are detected and the server fails to start with an error.

### Configure multiple servers at the same time with different configurations but with common ports. Ask why the server should work if one of the configurations isn't working
The server is designed to continue operating for valid configurations even if one fails, as each server is handled independently in the config parsing and listener setup.

---

## Siege & stress test

### Use siege with a GET method on an empty page, availability should be at least 99.5%
The server is event-driven and non-blocking, making it robust under load. Actual percentage depends on system resources and configuration.

### Check if there is no memory leak (you could use some tools like top)
The server uses Rust's ownership model and drops all connections cleanly (`Drop` implementation in `WebServer`), minimizing risk of leaks.

### Check if there is no hanging connection
Connections are closed on inactivity or errors; timeouts are managed in `cleanup_timeouts()`.

---

## References

- Main server logic: `src/server/mod.rs`
- Configuration parsing: `src/config/mod.rs`
- HTTP parsing: `src/http/request.rs`, `src/http/response.rs`
- CGI handling: `src/cgi/mod.rs`
- Session/cookie: `src/server/session.rs`
- Static files: `src/static_handler.rs`
- Entry point: `src/main.rs`
