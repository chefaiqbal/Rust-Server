use super::{HttpMethod, HttpVersion, Headers};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub uri: String,
    pub query_string: Option<String>,
    pub version: HttpVersion,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub query_params: HashMap<String, String>,
    pub cookies: HashMap<String, String>,
}

#[derive(Debug)]
pub enum ParseError {
    InvalidRequestLine,
    InvalidMethod,
    InvalidVersion,
    InvalidHeader,
    IncompleteRequest,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidRequestLine => write!(f, "Invalid request line"),
            ParseError::InvalidMethod => write!(f, "Invalid HTTP method"),
            ParseError::InvalidVersion => write!(f, "Invalid HTTP version"),
            ParseError::InvalidHeader => write!(f, "Invalid header"),
            ParseError::IncompleteRequest => write!(f, "Incomplete request"),
        }
    }
}

impl std::error::Error for ParseError {}

impl HttpRequest {
    /// Decode chunked transfer encoding body into a contiguous Vec<u8>
    fn decode_chunked_body(body: &[u8]) -> Result<Vec<u8>, ()> {
        let mut result = Vec::new();
        let mut pos = 0;
        let len = body.len();
        while pos < len {
            // Find the next CRLF
            let line_end = body[pos..].windows(2).position(|w| w == b"\r\n").ok_or(())? + pos;
            let size_str = std::str::from_utf8(&body[pos..line_end]).map_err(|_| ())?;
            let size = usize::from_str_radix(size_str.trim(), 16).map_err(|_| ())?;
            pos = line_end + 2;
            if size == 0 {
                // Last chunk
                break;
            }
            if pos + size > len {
                return Err(());
            }
            result.extend_from_slice(&body[pos..pos+size]);
            pos += size;
            // Skip CRLF after chunk
            if body.get(pos..pos+2) == Some(b"\r\n") {
                pos += 2;
            } else {
                return Err(());
            }
        }
        Ok(result)
    }
    pub fn new() -> Self {
        Self {
            method: HttpMethod::GET,
            uri: "/".to_string(),
            version: HttpVersion::default(),
            headers: HashMap::new(),
            body: Vec::new(),
            query_params: HashMap::new(),
            cookies: HashMap::new(),
            query_string: None,
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        let request_str = String::from_utf8_lossy(data);
        let parts: Vec<&str> = request_str.splitn(2, "\r\n\r\n").collect();
        
        if parts.is_empty() {
            return Err(ParseError::IncompleteRequest);
        }

        let header_part = parts[0];
        let body_part = if parts.len() > 1 { parts[1].as_bytes() } else { &[] };

        let mut lines = header_part.lines();
        
        // Parse request line
        let request_line = lines.next().ok_or(ParseError::InvalidRequestLine)?;
        let (method, uri, version) = Self::parse_request_line(request_line)?;
        
        // Parse headers
        let mut headers = HashMap::new();
        
        for line in lines {
            if line.is_empty() {
                break;
            }
            Self::parse_header_line(line, &mut headers)?;
        }

        // Parse query parameters
        let (path, query_string) = Self::parse_uri(&uri);
        let query_params = query_string.as_deref().map(Self::parse_query_string).unwrap_or_default();
        
        // Parse cookies
        let cookies = Self::parse_cookies(&headers);

        // If chunked, decode body accordingly
        let body = if headers.get("transfer-encoding").map(|v| v.to_lowercase().contains("chunked")).unwrap_or(false) {
            match Self::decode_chunked_body(body_part) {
                Ok(decoded) => decoded,
                Err(_) => return Err(ParseError::InvalidHeader),
            }
        } else {
            body_part.to_vec()
        };

        Ok(HttpRequest {
            method,
            uri: path,
            version,
            headers,
            body,
            query_params,
            cookies,
            query_string,
        })
    }

    fn parse_request_line(line: &str) -> Result<(HttpMethod, String, HttpVersion), ParseError> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(ParseError::InvalidRequestLine);
        }

        let method = HttpMethod::from_str(parts[0]).map_err(|_| ParseError::InvalidMethod)?;
        let uri = parts[1].to_string();
        let version = HttpVersion::from_str(parts[2]).map_err(|_| ParseError::InvalidVersion)?;

        Ok((method, uri, version))
    }

    fn parse_header_line(line: &str, headers: &mut Headers) -> Result<(), ParseError> {
        if let Some(colon_pos) = line.find(':') {
            let name = line[..colon_pos].trim().to_lowercase();
            let value = line[colon_pos + 1..].trim().to_string();
            headers.insert(name, value);
            Ok(())
        } else {
            Err(ParseError::InvalidHeader)
        }
    }

    fn parse_uri(uri_str: &str) -> (String, Option<String>) {
        if let Some(pos) = uri_str.find('?') {
            let uri = uri_str[..pos].to_string();
            let query_string = uri_str[pos + 1..].to_string();
            (uri, Some(query_string))
        } else {
            (uri_str.to_string(), None)
        }
    }

    fn parse_query_string(query_str: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();
        for pair in query_str.split('&') {
            if let Some(pos) = pair.find('=') {
                let key = Self::url_decode(&pair[..pos]);
                let value = Self::url_decode(&pair[pos + 1..]);
                params.insert(key, value);
            } else {
                params.insert(Self::url_decode(pair), "".to_string());
            }
        }
        params
    }

    fn parse_cookies(headers: &Headers) -> HashMap<String, String> {
        let mut cookies = HashMap::new();
        
        if let Some(cookie_header) = headers.get("cookie") {
            for cookie in cookie_header.split(';') {
                let cookie = cookie.trim();
                if let Some(eq_pos) = cookie.find('=') {
                    let name = cookie[..eq_pos].trim().to_string();
                    let value = cookie[eq_pos + 1..].trim().to_string();
                    cookies.insert(name, value);
                }
            }
        }
        
        cookies
    }

    fn url_decode(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '%' {
                if let (Some(h1), Some(h2)) = (chars.next(), chars.next()) {
                    if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                        result.push(byte as char);
                    } else {
                        result.push('%');
                        result.push(h1);
                        result.push(h2);
                    }
                } else {
                    result.push('%');
                }
            } else if ch == '+' {
                result.push(' ');
            } else {
                result.push(ch);
            }
        }
        
        result
    }

    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_lowercase())
    }

    pub fn has_header(&self, name: &str) -> bool {
        self.headers.contains_key(&name.to_lowercase())
    }

    pub fn content_length(&self) -> Option<usize> {
        self.get_header("content-length")
            .and_then(|v| v.parse().ok())
    }

    pub fn content_type(&self) -> Option<&String> {
        self.get_header("content-type")
    }

    pub fn host(&self) -> Option<&String> {
        self.get_header("host")
    }

    pub fn user_agent(&self) -> Option<&String> {
        self.get_header("user-agent")
    }

    pub fn is_keep_alive(&self) -> bool {
        if let Some(connection) = self.get_header("connection") {
            connection.to_lowercase() == "keep-alive"
        } else {
            // HTTP/1.1 defaults to keep-alive
            self.version.major == 1 && self.version.minor >= 1
        }
    }

    pub fn expects_continue(&self) -> bool {
        if let Some(expect) = self.get_header("expect") {
            expect.to_lowercase() == "100-continue"
        } else {
            false
        }
    }

    pub fn is_chunked(&self) -> bool {
        if let Some(encoding) = self.get_header("transfer-encoding") {
            encoding.to_lowercase().contains("chunked")
        } else {
            false
        }
    }

    pub fn get_cookie(&self, name: &str) -> Option<&String> {
        self.cookies.get(name)
    }

    pub fn get_query_param(&self, name: &str) -> Option<&String> {
        self.query_params.get(name)
    }
}

impl Default for HttpRequest {
    fn default() -> Self {
        Self::new()
    }
}