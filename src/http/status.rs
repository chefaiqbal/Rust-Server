use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusCode {
    // 1xx Informational
    Continue = 100,
    SwitchingProtocols = 101,
    
    // 2xx Success
    Ok = 200,
    Created = 201,
    Accepted = 202,
    NoContent = 204,
    
    // 3xx Redirection
    MovedPermanently = 301,
    Found = 302,
    NotModified = 304,
    
    // 4xx Client Error
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    RequestTimeout = 408,
    LengthRequired = 411,
    PayloadTooLarge = 413,
    UriTooLong = 414,
    
    // 5xx Server Error
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    HttpVersionNotSupported = 505,
}

impl StatusCode {
    pub fn reason_phrase(&self) -> &'static str {
        match self {
            StatusCode::Continue => "Continue",
            StatusCode::SwitchingProtocols => "Switching Protocols",
            StatusCode::Ok => "OK",
            StatusCode::Created => "Created",
            StatusCode::Accepted => "Accepted",
            StatusCode::NoContent => "No Content",
            StatusCode::MovedPermanently => "Moved Permanently",
            StatusCode::Found => "Found",
            StatusCode::NotModified => "Not Modified",
            StatusCode::BadRequest => "Bad Request",
            StatusCode::Unauthorized => "Unauthorized",
            StatusCode::Forbidden => "Forbidden",
            StatusCode::NotFound => "Not Found",
            StatusCode::MethodNotAllowed => "Method Not Allowed",
            StatusCode::RequestTimeout => "Request Timeout",
            StatusCode::LengthRequired => "Length Required",
            StatusCode::PayloadTooLarge => "Payload Too Large",
            StatusCode::UriTooLong => "URI Too Long",
            StatusCode::InternalServerError => "Internal Server Error",
            StatusCode::NotImplemented => "Not Implemented",
            StatusCode::BadGateway => "Bad Gateway",
            StatusCode::ServiceUnavailable => "Service Unavailable",
            StatusCode::HttpVersionNotSupported => "HTTP Version Not Supported",
        }
    }

    pub fn is_informational(&self) -> bool {
        (*self as u16) >= 100 && (*self as u16) < 200
    }

    pub fn is_success(&self) -> bool {
        (*self as u16) >= 200 && (*self as u16) < 300
    }

    pub fn is_redirection(&self) -> bool {
        (*self as u16) >= 300 && (*self as u16) < 400
    }

    pub fn is_client_error(&self) -> bool {
        (*self as u16) >= 400 && (*self as u16) < 500
    }

    pub fn is_server_error(&self) -> bool {
        (*self as u16) >= 500 && (*self as u16) < 600
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", *self as u16, self.reason_phrase())
    }
}

impl From<u16> for StatusCode {
    fn from(code: u16) -> Self {
        match code {
            100 => StatusCode::Continue,
            101 => StatusCode::SwitchingProtocols,
            200 => StatusCode::Ok,
            201 => StatusCode::Created,
            202 => StatusCode::Accepted,
            204 => StatusCode::NoContent,
            301 => StatusCode::MovedPermanently,
            302 => StatusCode::Found,
            304 => StatusCode::NotModified,
            400 => StatusCode::BadRequest,
            401 => StatusCode::Unauthorized,
            403 => StatusCode::Forbidden,
            404 => StatusCode::NotFound,
            405 => StatusCode::MethodNotAllowed,
            408 => StatusCode::RequestTimeout,
            411 => StatusCode::LengthRequired,
            413 => StatusCode::PayloadTooLarge,
            414 => StatusCode::UriTooLong,
            500 => StatusCode::InternalServerError,
            501 => StatusCode::NotImplemented,
            502 => StatusCode::BadGateway,
            503 => StatusCode::ServiceUnavailable,
            505 => StatusCode::HttpVersionNotSupported,
            _ => StatusCode::InternalServerError,
        }
    }
}