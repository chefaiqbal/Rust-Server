use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub listen: u16,
    pub server_name: String,
    pub client_max_body_size: usize,
    pub error_pages: HashMap<u16, String>,
    pub routes: Vec<RouteConfig>,
}

#[derive(Debug, Clone)]
pub struct ServerLocation {
    pub path: String,
    pub root: Option<String>,
    pub alias: Option<String>,
    pub index: Option<Vec<String>>,
    pub autoindex: Option<bool>,
    pub allow_methods: Option<Vec<String>>,
    pub error_page: Option<HashMap<u16, String>>,
    pub client_max_body_size: Option<usize>,
    pub cgi_pass: Option<String>,
    pub cgi_extension: Option<String>,
    pub upload_store: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RouteConfig {
    pub path: String,
    pub methods: Vec<String>,
    pub root: Option<String>,
    pub index: Option<String>,
    pub autoindex: bool,
    pub redirect: Option<String>,
    pub cgi_pass: Option<String>,
    pub cgi_extension: Option<String>,
    pub upload_store: Option<String>,
    pub default_file: Option<String>,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    fn parse(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut servers = Vec::new();
        let mut current_server: Option<ServerConfig> = None;
        let mut current_route: Option<RouteConfig> = None;
        let mut brace_level = 0;

        for line in content.lines() {
            let line = line.trim();
            
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line == "server {" {
                current_server = Some(ServerConfig::default());
                brace_level = 1;
                continue;
            }

            if line.starts_with("location ") && line.ends_with(" {") {
                if let Some(path) = Self::extract_location_path(line) {
                    current_route = Some(RouteConfig::new(path));
                    brace_level = 2;
                }
                continue;
            }

            if line == "}" {
                brace_level -= 1;
                if brace_level == 1 && current_route.is_some() {
                    // End of location block
                    if let (Some(ref mut server), Some(route)) = (&mut current_server, current_route.take()) {
                        server.routes.push(route);
                    }
                } else if brace_level == 0 && current_server.is_some() {
                    // End of server block
                    if let Some(server) = current_server.take() {
                        servers.push(server);
                    }
                }
                continue;
            }

            // Parse directives
            if let Some(ref mut server) = current_server {
                if brace_level == 1 {
                    // Server-level directive
                    Self::parse_server_directive(server, line)?;
                } else if brace_level == 2 {
                    // Location-level directive
                    if let Some(ref mut route) = current_route {
                        Self::parse_location_directive(route, line)?;
                    }
                }
            }
        }

        // Add any remaining server
        if let Some(server) = current_server {
            servers.push(server);
        }

        Ok(Config { servers })
    }

    fn extract_location_path(line: &str) -> Option<String> {
        // Extract path from "location /path {"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            Some(parts[1].to_string())
        } else {
            None
        }
    }

    fn parse_server_directive(server: &mut ServerConfig, line: &str) -> Result<(), Box<dyn std::error::Error>> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            return Ok(());
        }

        match parts[0] {
            "listen" => {
                let port_str = parts[1].trim_end_matches(';');
                server.listen = port_str.parse()?;
            }
            "server_name" => {
                server.server_name = parts[1].trim_end_matches(';').to_string();
            }
            "client_max_body_size" => {
                let size_str = parts[1].trim_end_matches(';');
                server.client_max_body_size = Self::parse_size(size_str)?;
            }
            "error_page" => {
                if parts.len() >= 3 {
                    let status: u16 = parts[1].parse()?;
                    let page = parts[2].trim_end_matches(';').to_string();
                    server.error_pages.insert(status, page);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn parse_location_directive(route: &mut RouteConfig, line: &str) -> Result<(), Box<dyn std::error::Error>> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "allow_methods" => {
                let methods: Vec<String> = parts[1..]
                    .iter()
                    .map(|s| s.trim_end_matches(';').to_string())
                    .collect();
                route.methods = methods;
            }
            "root" => {
                if parts.len() >= 2 {
                    route.root = Some(parts[1].trim_end_matches(';').to_string());
                }
            }
            "index" => {
                if parts.len() >= 2 {
                    route.index = Some(parts[1].trim_end_matches(';').to_string());
                }
            }
            "autoindex" => {
                if parts.len() >= 2 {
                    route.autoindex = parts[1].trim_end_matches(';') == "on";
                }
            }
            "return" => {
                if parts.len() >= 3 {
                    // Skip the status code, just get the URL
                    route.redirect = Some(parts[2].trim_end_matches(';').to_string());
                }
            }
            "cgi_pass" => {
                if parts.len() >= 2 {
                    route.cgi_pass = Some(parts[1].trim_end_matches(';').to_string());
                }
            }
            "upload_store" => {
                if parts.len() >= 2 {
                    route.upload_store = Some(parts[1].trim_end_matches(';').to_string());
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn parse_size(size_str: &str) -> Result<usize, Box<dyn std::error::Error>> {
        let size_str = size_str.to_uppercase();
        
        if let Some(num_str) = size_str.strip_suffix('K') {
            Ok(num_str.parse::<usize>()? * 1024)
        } else if let Some(num_str) = size_str.strip_suffix('M') {
            Ok(num_str.parse::<usize>()? * 1024 * 1024)
        } else if let Some(num_str) = size_str.strip_suffix('G') {
            Ok(num_str.parse::<usize>()? * 1024 * 1024 * 1024)
        } else {
            // No suffix, assume bytes
            Ok(size_str.parse::<usize>()?)
        }
    }
}

impl ServerConfig {
    pub fn socket_addr(&self) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let addr = format!("127.0.0.1:{}", self.listen);
        Ok(addr.parse()?)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: 80,
            server_name: "localhost".to_string(),
            client_max_body_size: 1024 * 1024, // 1MB default
            error_pages: HashMap::new(),
            routes: Vec::new(),
        }
    }
}

impl RouteConfig {
    fn new(path: String) -> Self {
        Self {
            path,
            methods: vec!["GET".to_string()],
            root: None,
            index: None,
            autoindex: false,
            redirect: None,
            cgi_pass: None,
            cgi_extension: None,
            upload_store: None,
            default_file: None,
        }
    }
}