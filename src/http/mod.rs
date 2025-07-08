use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

pub mod request;
pub mod response;
pub mod status;

pub use request::HttpRequest;
pub use response::HttpResponse;
pub use status::StatusCode;

#[derive(Debug, Clone, PartialEq)]
pub enum HttpMethod {
    GET,
    POST,
    DELETE,
    HEAD,
    PUT,
    OPTIONS,
}

impl FromStr for HttpMethod {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::GET),
            "POST" => Ok(HttpMethod::POST),
            "DELETE" => Ok(HttpMethod::DELETE),
            "HEAD" => Ok(HttpMethod::HEAD),
            "PUT" => Ok(HttpMethod::PUT),
            "OPTIONS" => Ok(HttpMethod::OPTIONS),
            _ => Err(()),
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let method_str = match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::HEAD => "HEAD",
            HttpMethod::PUT => "PUT",
            HttpMethod::OPTIONS => "OPTIONS",
        };
        write!(f, "{}", method_str)
    }
}

#[derive(Debug, Clone)]
pub struct HttpVersion {
    pub major: u8,
    pub minor: u8,
}

impl Default for HttpVersion {
    fn default() -> Self {
        HttpVersion { major: 1, minor: 1 }
    }
}

impl fmt::Display for HttpVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP/{}.{}", self.major, self.minor)
    }
}

impl FromStr for HttpVersion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(version_part) = s.strip_prefix("HTTP/") {
            let parts: Vec<&str> = version_part.split('.').collect();
            if parts.len() == 2 {
                if let (Ok(major), Ok(minor)) = (parts[0].parse(), parts[1].parse()) {
                    return Ok(HttpVersion { major, minor });
                }
            }
        }
        Err(())
    }
}

pub type Headers = HashMap<String, String>;