use crate::config::{RouteConfig, ServerConfig};
use crate::http::{HttpRequest, HttpResponse};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::env;
use log::debug;

pub struct StaticFileHandler {
    server_root: PathBuf,
}

impl StaticFileHandler {
    pub fn new(server_config: &ServerConfig) -> Self {
        // Get the current directory where the server is running from
        let current_dir = match env::current_dir() {
    Ok(dir) => dir,
    Err(e) => {
        log::error!("Failed to get current directory: {}", e);
        PathBuf::from(".")
    }
};
        debug!("Current directory: {:?}", current_dir);
        
        // Default to current directory if no root is specified in the config
        let root = server_config.routes
            .iter()
            .find(|r| r.path == "/")
            .and_then(|r| r.root.as_ref())
            .map(PathBuf::from)
            .unwrap_or_else(|| current_dir.clone());

        debug!("Root from config: {:?}", root);

        // If the path is relative, make it absolute relative to the current directory
        let server_root = if root.is_relative() {
            let abs_path = current_dir.join(&root);
            debug!("Converted relative path to absolute: {:?}", abs_path);
            abs_path
        } else {
            root
        };

        debug!("Server root: {:?}", server_root);

        Self { server_root }
    }

    /// Special demo endpoints:
    ///   - /chunked-demo returns a chunked response
    ///   - /normal-demo returns a Content-Length response
    pub fn handle_request(&self, request: &crate::http::HttpRequest, server_config: &crate::config::ServerConfig) -> crate::http::HttpResponse {
        use crate::http::HttpMethod;

        // Get the path from the URI, handling query parameters
        let path = match request.uri.split('?').next() {
            Some(path) => path,
            None => return HttpResponse::bad_request(),
        };

        // --- Demo endpoints for chunked vs normal responses ---
        if path == "/chunked-demo" {
            let mut resp = HttpResponse::ok();
            resp.set_header("Content-Type", "text/html");
            resp.set_header("transfer-encoding", "chunked");
            let html = r#"
                <html><body>
                <h1>Chunked Demo Page</h1>
                <p>This response is sent with <b>Transfer-Encoding: chunked</b>.</p>
                </body></html>
            "#;
            resp.set_body(html.as_bytes());
            return resp;
        }
        if path == "/normal-demo" {
            let mut resp = HttpResponse::ok();
            resp.set_header("Content-Type", "text/html");
            let html = r#"
                <html><body>
                <h1>Normal Demo Page</h1>
                <p>This response is sent with <b>Content-Length</b>.</p>
                </body></html>
            "#;
            resp.set_body(html.as_bytes());
            return resp;
        }

        // Find the best matching location block
        let location = self.find_best_location(path, server_config);
        
        debug!("Found location for path '{}': path='{}', methods={:?}", path, location.path, location.methods);

        // CHECK FOR EMPTY METHODS FIRST - This is the key fix for 403 Forbidden
        if location.methods.is_empty() {
            debug!("Empty methods for location '{}', returning 403 Forbidden", location.path);
            
            // Try to serve custom 403 error page
            if let Some(error_page_path) = server_config.error_pages.get(&403) {
                let error_path = if error_page_path.starts_with('/') {
                    error_page_path.trim_start_matches('/').to_string()
                } else {
                    error_page_path.clone()
                };
                let full_path = self.server_root.join(&error_path);
                debug!("Looking for 403 error page at: {:?}", full_path);
                
                if let Ok(content) = std::fs::read(&full_path) {
                    let mut response = HttpResponse::new(crate::http::StatusCode::Forbidden);
                    response.set_body(&content);
                    response.set_header("Content-Type", "text/html");
                    debug!("Serving custom 403 page");
                    return response;
                }
            }
            
            // Fallback to default 403 response
            debug!("No custom 403 page found, using default");
            return HttpResponse::forbidden();
        }

        // Check if the method is allowed for this location
        if !location.methods.iter().any(|m| m == &request.method.to_string()) {
            let error_page = server_config.error_pages.get(&405).map(|s| s.as_str());
            return HttpResponse::method_not_allowed_custom(error_page);
        }

        // Serve upload form on GET if upload_store is set
        if request.method == HttpMethod::GET {
            if let Some(_upload_dir) = &location.upload_store {
                // Serve a simple HTML upload form
                let html = format!(r#"
                    <html><body>
                    <h1>Upload a file</h1>
                    <form method="POST" enctype="multipart/form-data" action="{}">
                        <input type="file" name="file" required />
                        <button type="submit">Upload</button>
                    </form>
                    </body></html>
                "#, path);
                let mut resp = HttpResponse::ok();
                resp.set_header("Content-Type", "text/html");
                resp.set_body(html.as_bytes());
                return resp;
            }
        }

        // Handle file upload if POST and upload_store is set
        if request.method == HttpMethod::POST {
            if let Some(upload_dir) = &location.upload_store {
                // Only accept multipart/form-data
                let content_type = request.headers.get("content-type").map(|s| s.as_str()).unwrap_or("");
                if let Some(boundary) = Self::extract_boundary(content_type) {
                    match Self::save_multipart_file(&request.body, &boundary, upload_dir) {
                        Ok(Some(filename)) => {
                            // Show a link or image preview
                            let file_url = format!("{}/{}", path.trim_end_matches('/'), filename);
                            let mut html = String::from("<html><body><h1>Upload successful!</h1>");
                            if filename.ends_with(".png") || filename.ends_with(".jpg") || filename.ends_with(".jpeg") || filename.ends_with(".gif") {
                                html.push_str(&format!("<img src='{}' style='max-width:400px;'/><br>", file_url));
                            }
                            html.push_str(&format!("<a href='{}'>View uploaded file</a>", file_url));
                            html.push_str("</body></html>");
                            let mut resp = HttpResponse::ok();
                            resp.set_header("Content-Type", "text/html");
                            resp.set_body(html.as_bytes());
                            return resp;
                        }
                        Ok(None) => {
                            let mut resp = HttpResponse::bad_request();
                            resp.set_body(b"No file found in upload");
                            return resp;
                        }
                        Err(e) => {
                            let mut resp = HttpResponse::internal_server_error();
                            resp.set_body(format!("Upload error: {}", e).as_bytes());
                            return resp;
                        }
                    }
                } else {
                    let mut resp = HttpResponse::bad_request();
                    resp.set_body(b"Missing or invalid Content-Type: multipart/form-data");
                    return resp;
                }
            }
        }

        // Handle DELETE method for file deletion
        if request.method == HttpMethod::DELETE {
            if let Some(_upload_dir) = &location.upload_store {
                // Only allow deletion in upload directories for security
                return self.handle_delete_request(path, &location);
            } else {
                // For security, only allow DELETE in specific directories
                let fs_path = self.resolve_path(path, &location);
                
                // Security check: only allow deletion of files in uploads directory
                if fs_path.to_string_lossy().contains("/uploads/") {
                    return self.handle_delete_request(path, &location);
                } else {
                    // Deny deletion outside uploads directory
                    return HttpResponse::forbidden();
                }
            }
        }

        // Only handle GET and HEAD methods for static files
        if request.method != HttpMethod::GET && request.method != HttpMethod::HEAD {
            let error_page = server_config.error_pages.get(&405).map(|s| s.as_str());
            return HttpResponse::method_not_allowed_custom(error_page);
        }

        // Build the full filesystem path (only after all checks pass)
        let fs_path = self.resolve_path(path, &location);
        
        // Security check: Prevent directory traversal
        if !fs_path.starts_with(&self.server_root) {
            return HttpResponse::forbidden();
        }
        
        // Check if the file exists and is accessible
        match fs::metadata(&fs_path) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    self.handle_directory(&fs_path, &location, request)
                } else {
                    self.serve_file(&fs_path, &request, &metadata)
                }
            }
            Err(_) => {
                debug!("File not found: {}", fs_path.display());
                HttpResponse::not_found()
            }
        }
    }

    fn extract_boundary(content_type: &str) -> Option<String> {
        // Example: Content-Type: multipart/form-data; boundary=----WebKitFormBoundaryePkpFF7tjBAqx29L
        content_type.split(';')
            .find_map(|part| {
                let part = part.trim();
                if part.starts_with("boundary=") {
                    Some(part[9..].trim_matches('"').to_string())
                } else {
                    None
                }
            })
    }

    fn save_multipart_file(body: &[u8], boundary: &str, upload_dir: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
        use std::fs;
        use std::path::Path;
        let boundary_marker = format!("--{}", boundary);
        let body_str = String::from_utf8_lossy(body);
        let mut filename = None;
        let mut filedata = None;
        for part in body_str.split(&boundary_marker) {
            // Look for Content-Disposition with filename
            if let Some(disposition) = part.find("Content-Disposition:") {
                if let Some(fname_start) = part.find("filename=\"") {
                    let fname_end = part[fname_start+10..].find('"').map(|i| fname_start+10+i).unwrap_or(part.len());
                    let fname = &part[fname_start+10..fname_end];
                    if !fname.is_empty() {
                        filename = Some(fname.to_string());
                        // Find start of file data (after double CRLF)
                        if let Some(data_start) = part.find("\r\n\r\n") {
                            let data = &part[data_start+4..];
                            // Remove trailing CRLF-- if present
                            let data = data.trim_end_matches(|c| c == '\r' || c == '\n' || c == '-').as_bytes();
                            filedata = Some(data.to_vec());
                        }
                    }
                }
            }
        }
        if let (Some(fname), Some(data)) = (filename, filedata) {
            let dir = Path::new(upload_dir);
            fs::create_dir_all(dir)?;
            let save_path = dir.join(&fname);
            let mut file = File::create(&save_path)?;
            file.write_all(&data)?;
            Ok(Some(fname))
        } else {
            Ok(None)
        }
    }


    fn find_best_location<'a>(&self, path: &str, server_config: &'a ServerConfig) -> &'a RouteConfig {
        server_config.routes
            .iter()
            .filter(|r| path.starts_with(&r.path))
            .max_by_key(|r| r.path.len())
            .unwrap_or_else(|| &server_config.routes[0]) // Default to first route
    }

    fn resolve_path(&self, uri_path: &str, location: &RouteConfig) -> PathBuf {
        // Start with the server root
        let mut path_buf = self.server_root.clone();
        debug!("Initial path_buf: {:?}", path_buf);
        
        // If root is specified in the location, use it instead of server_root
        if let Some(root) = &location.root {
            let root_path = PathBuf::from(root);
            debug!("Root from location: {:?}", root_path);
            
            if root_path.is_absolute() {
                path_buf = root_path;
                debug!("Using absolute path from config: {:?}", path_buf);
            } else {
                // For relative paths, resolve relative to the current directory, not server_root
                let current_dir = match env::current_dir() {
    Ok(dir) => dir,
    Err(e) => {
        log::error!("Failed to get current directory: {}", e);
        PathBuf::from(".")
    }
};
                path_buf = current_dir.join(root_path);
                debug!("Resolved relative path: {:?}", path_buf);
            }
        }
        
        // Append the URI path (relative to the location path)
        if let Some(stripped) = uri_path.strip_prefix(&location.path) {
            let trimmed = stripped.trim_start_matches('/');
            if !trimmed.is_empty() {
                debug!("Stripped path: '{}' -> '{}'", uri_path, trimmed);
                path_buf.push(trimmed);
            }
        } else if !uri_path.is_empty() && uri_path != "/" {
            let trimmed = uri_path.trim_start_matches('/');
            debug!("Using full path: '{}' -> '{}'", uri_path, trimmed);
            path_buf.push(trimmed);
        }
        
        debug!("Path before canonicalize: {:?}", path_buf);
        
        // Normalize the path (resolve . and ..)
        let normalized = path_buf.canonicalize().unwrap_or_else(|_| {
            debug!("Failed to canonicalize path: {:?}", path_buf);
            path_buf
        });
        
        debug!("Normalized path: {:?}", normalized);
        
        // Security check: Ensure the path is within the server root
        if !normalized.starts_with(&self.server_root) {
            debug!("Security check failed: Path '{}' is not under server root '{}'", 
                  normalized.display(), self.server_root.display());
            return self.server_root.join("403.html");
        }
        
        normalized
    }

    fn serve_file(&self, path: &Path, _request: &HttpRequest, metadata: &std::fs::Metadata) -> HttpResponse {
        match fs::read(path) {
            Ok(content) => {
                let mut response = HttpResponse::ok();
                
                // Set Content-Type based on file extension
                let mime_type = path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase())
                    .as_deref()
                    .map(|ext| match ext {
                        "html" | "htm" => "text/html",
                        "css" => "text/css",
                        "js" => "application/javascript",
                        "json" => "application/json",
                        "jpg" | "jpeg" => "image/jpeg",
                        "png" => "image/png",
                        "gif" => "image/gif",
                        "svg" => "image/svg+xml",
                        "pdf" => "application/pdf",
                        "txt" => "text/plain",
                        _ => "application/octet-stream",
                    })
                    .unwrap_or("application/octet-stream");
                
                response.set_header("Content-Type", mime_type);
                response.set_header("Content-Length", &content.len().to_string());
                response.set_header("Last-Modified", &self.http_date(metadata.modified().unwrap_or_else(|_| SystemTime::now())));
                response.set_body(&content);
                response
            }
            Err(e) => {
                use std::io::ErrorKind;
                debug!("Failed to read file: {}: {}", path.display(), e);
                if e.kind() == ErrorKind::PermissionDenied {
                    HttpResponse::forbidden()
                } else {
                    HttpResponse::not_found()
                }
            }
        }
    }

    fn handle_directory(&self, path: &Path, location: &RouteConfig, _request: &HttpRequest) -> HttpResponse {
        // Check for index file if specified
        if let Some(index) = &location.index {
            let index_path = path.join(index);
            if let Ok(metadata) = fs::metadata(&index_path) {
                if metadata.is_file() {
                    return self.serve_file(&index_path, _request, &metadata);
                }
            }
        }

        // If autoindex is on, generate directory listing
        if location.autoindex {
            self.generate_directory_listing(path)
        } else {
            HttpResponse::forbidden()
        }
    }

    fn generate_directory_listing(&self, path: &Path) -> HttpResponse {
        let mut html = String::new();
        
        // Simple HTML header
        html.push_str("<!DOCTYPE html><html><head><title>Index of ");
        html.push_str(&path.file_name().unwrap_or_default().to_string_lossy());
        html.push_str("</title></head><body>");
        html.push_str("<h1>Index of ");
        html.push_str(&path.file_name().unwrap_or_default().to_string_lossy());
        html.push_str("</h1><hr><pre>");
        
        // Add parent directory link if not at root
        if path != &self.server_root {
            if path.parent().is_some() {
                html.push_str("<a href='../'>back</a>                             -\n");
            }
        }
        
        // Collect and sort directory entries
        let mut entries = Vec::new();
        if let Ok(dir_entries) = fs::read_dir(path) {
            for entry in dir_entries.filter_map(Result::ok) {
                if let (Ok(metadata), Ok(file_name)) = (entry.metadata(), entry.file_name().into_string()) {
                    // Skip hidden files
                    if file_name.starts_with('.') {
                        continue;
                    }
                    entries.push((file_name, metadata));
                }
            }
        }
        
        // Sort entries: directories first, then files, both alphabetically
        entries.sort_by(|a, b| {
            match (a.1.is_dir(), b.1.is_dir()) {
                (true, false) => std::cmp::Ordering::Less,  // directories first
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.0.cmp(&b.0)  // alphabetical within each group
            }
        });
        
        // Generate listing
        for (file_name, metadata) in entries {
            let display_name = if metadata.is_dir() {
                format!("{}/", file_name)
            } else {
                file_name.clone()
            };
            
            let size = if metadata.is_dir() {
                "-".to_string()
            } else {
                metadata.len().to_string()
            };
            
            // Format similar to Apache/nginx directory listing
            html.push_str(&format!(
                "<a href='{}'>{}</a>{} {}\n",
                file_name,
                display_name,
                " ".repeat(50_usize.saturating_sub(display_name.len())),
                size
            ));
        }
        
        // Close HTML
        html.push_str("</pre><hr></body></html>");
        
        let mut response = HttpResponse::ok();
        response.set_body(html.as_bytes());
        response.set_header("Content-Type", "text/html");
        response
    }
    
    fn format_file_size(&self, bytes: u64) -> String {
        // Simple size formatting - can be improved if needed
        format!("{} bytes", bytes)
    }

    fn http_date(&self, time: SystemTime) -> String {
        use std::time::UNIX_EPOCH;
        
        // Convert SystemTime to seconds since epoch
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or_else(|_| std::time::Duration::new(0, 0));
        
        // Simple timestamp - for a production server, use a proper date formatting library
        let secs = duration.as_secs();
        format!("{}", secs)
    }

    fn handle_delete_request(&self, path: &str, location: &RouteConfig) -> HttpResponse {
        let fs_path = self.resolve_path(path, location);
        
        // Security check: Prevent directory traversal
        if !fs_path.starts_with(&self.server_root) {
            return HttpResponse::forbidden();
        }
        
        // Check if file exists
        match std::fs::metadata(&fs_path) {
            Ok(metadata) => {
                if metadata.is_file() {
                    // Attempt to delete the file
                    match std::fs::remove_file(&fs_path) {
                        Ok(_) => {
                            let mut response = HttpResponse::ok();
                            let html = format!(
                                "<html><body><h1>File Deleted</h1><p>Successfully deleted: {}</p><a href='/'>Go Home</a></body></html>",
                                path
                            );
                            response.set_body(html.as_bytes());
                            response.set_header("Content-Type", "text/html");
                            response
                        }
                        Err(_) => {
                            HttpResponse::forbidden() // Could not delete (permissions, etc.)
                        }
                    }
                } else {
                    // Cannot delete directories
                    HttpResponse::forbidden()
                }
            }
            Err(_) => {
                HttpResponse::not_found() // File doesn't exist
            }
        }
    }
}
