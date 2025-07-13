use super::{Headers, HttpVersion, StatusCode};
use std::collections::HashMap;
use std::fmt::Write;
use std::fs;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub version: HttpVersion,
    pub status: StatusCode,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: StatusCode) -> Self {
        let mut headers = HashMap::new();
        headers.insert("server".to_string(), "webserv/1.0".to_string());
        headers.insert("date".to_string(), Self::current_date());
        
        Self {
            version: HttpVersion::default(),
            status,
            headers,
            body: Vec::new(),
        }
    }

    pub fn ok() -> Self {
        Self::new(StatusCode::Ok)
    }

    pub fn not_found() -> Self {
        let mut response = Self::new(StatusCode::NotFound);
        response.set_body(b"<html><body><h1>404 Not Found</h1></body></html>");
        response.set_header("content-type", "text/html");
        response
    }

    pub fn bad_request() -> Self {
        let mut response = Self::new(StatusCode::BadRequest);
        response.set_body(b"<html><body><h1>400 Bad Request</h1></body></html>");
        response.set_header("content-type", "text/html");
        response
    }

    pub fn internal_server_error() -> Self {
        let mut response = Self::new(StatusCode::InternalServerError);
        response.set_body(b"<html><body><h1>500 Internal Server Error</h1></body></html>");
        response.set_header("content-type", "text/html");
        response
    }

    pub fn method_not_allowed_custom(error_page: Option<&str>) -> Self {
        if let Some(path) = error_page {
            if let Ok(metadata) = std::fs::metadata(path) {
                if !metadata.is_dir() {
                    if let Ok(content) = std::fs::read(path) {
                        let mut custom_response = Self::new(StatusCode::MethodNotAllowed);
                        custom_response.set_body(&content);
                        custom_response.set_header("content-type", "text/html");
                        return custom_response;
                    }
                }
            }
        }
        let mut response = Self::new(StatusCode::MethodNotAllowed);
        response.set_body(b"<html><body><h1>405 Method Not Allowed</h1></body></html>");
        response.set_header("content-type", "text/html");
        response
    }

    pub fn forbidden() -> Self {
        let mut response = Self::new(StatusCode::Forbidden);
        response.set_body(b"<html><body><h1>403 Forbidden</h1></body></html>");
        response.set_header("content-type", "text/html");
        response
    }

    pub fn payload_too_large() -> Self {
        let mut response = Self::new(StatusCode::PayloadTooLarge);
        response.set_body(b"<html><body><h1>413 Payload Too Large</h1></body></html>");
        response.set_header("content-type", "text/html");
        response
    }

    pub fn redirect(location: &str) -> Self {
        let mut response = Self::new(StatusCode::Found);
        response.set_header("location", location);
        response.set_body(b"<html><body><h1>302 Found</h1></body></html>");
        response.set_header("content-type", "text/html");
        response
    }

    pub fn set_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_lowercase(), value.to_string());
    }

    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_lowercase())
    }

    pub fn set_body(&mut self, body: &[u8]) {
        self.body = body.to_vec();
        self.set_header("content-length", &self.body.len().to_string());
    }

    pub fn set_body_string(&mut self, body: &str) {
        self.set_body(body.as_bytes());
    }

    pub fn set_cookie(&mut self, name: &str, value: &str, max_age: Option<u64>, path: Option<&str>) {
        let mut cookie = format!("{}={}", name, value);
        
        if let Some(age) = max_age {
            write!(&mut cookie, "; Max-Age={}", age).unwrap();
        }
        
        if let Some(path) = path {
            write!(&mut cookie, "; Path={}", path).unwrap();
        }
        
        self.headers.insert("set-cookie".to_string(), cookie);
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = Vec::new();
        
        // Status line
        let status_line = format!("{} {}\r\n", self.version, self.status);
        response.extend_from_slice(status_line.as_bytes());
        
        // Headers
        for (name, value) in &self.headers {
            let header_line = format!("{}: {}\r\n", name, value);
            response.extend_from_slice(header_line.as_bytes());
        }
        
        // Empty line to separate headers from body
        response.extend_from_slice(b"\r\n");
        
        // Body
        response.extend_from_slice(&self.body);
        
        response
    }

    fn current_date() -> String {
        // For now, return a simple date format
        // In a real implementation, you'd use a proper date/time library
        "Mon, 01 Jan 2024 00:00:00 GMT".to_string()
    }

    pub fn content_type_from_extension(extension: &str) -> &'static str {
        match extension.to_lowercase().as_str() {
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" => "application/javascript",
            "json" => "application/json",
            "xml" => "application/xml",
            "txt" => "text/plain",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self::new(StatusCode::Ok)
    }
}