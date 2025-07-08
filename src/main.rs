use std::env;
use std::process;

mod config;
mod server;
mod http;
mod cgi;
mod utils;

use config::Config;
use server::WebServer;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    let config_path = if args.len() > 1 {
        &args[1]
    } else {
        "config/webserv.conf"
    };

    let config = match Config::from_file(config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            process::exit(1);
        }
    };

    let mut server = WebServer::new(config);
    
    if let Err(e) = server.run() {
        eprintln!("Server error: {}", e);
        process::exit(1);
    }
}