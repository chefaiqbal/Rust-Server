use crate::config::{RouteConfig, ServerConfig};
use crate::http::{HttpRequest, HttpResponse};
use std::fs;
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
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
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

    pub fn handle_request(&self, request: &HttpRequest, server_config: &ServerConfig) -> HttpResponse {
        // Only handle GET and HEAD methods for static files
        if request.method != crate::http::HttpMethod::GET && request.method != crate::http::HttpMethod::HEAD {
            return HttpResponse::method_not_allowed();
        }

        // Get the path from the URI, handling query parameters
        let path = match request.uri.split('?').next() {
            Some(path) => path,
            None => return HttpResponse::bad_request(),
        };

        // Find the best matching location block
        let location = self.find_best_location(path, server_config);
        
        // Check if the method is allowed for this location
        if !location.methods.is_empty() {
            if !location.methods.iter().any(|m| m == &request.method.to_string()) {
                return HttpResponse::method_not_allowed();
            }
        }

        // Build the full filesystem path
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
                let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
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
            Err(_) => {
                debug!("Failed to read file: {}", path.display());
                HttpResponse::not_found()
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
        html.push_str("<html><head><title>Directory Listing</title></head><body>");
        html.push_str("<h1>Directory Listing</h1><ul>");
        
        // Add parent directory link if not at root
        if path != &self.server_root {
            if path.parent().is_some() {
                html.push_str("<li><a href='../'>.. (Parent Directory)</a></li>");
            }
        }
        
        // List directory contents
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.filter_map(Result::ok) {
                if let (Ok(metadata), Ok(file_name)) = (entry.metadata(), entry.file_name().into_string()) {
                    // Skip hidden files
                    if file_name.starts_with('.') {
                        continue;
                    }
                    
                    let display_name = if metadata.is_dir() {
                        format!("{}/", file_name)
                    } else {
                        file_name.clone()
                    };
                    
                    html.push_str(&format!("<li><a href='{}'>{}</a></li>", file_name, display_name));
                }
            }
        }
        
        // Close HTML
        html.push_str("</ul></body></html>");
        
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
}
