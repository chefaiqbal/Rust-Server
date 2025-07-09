use std::env;
use std::process;

mod config;
mod server;
mod http;
mod cgi;
mod utils;
mod static_handler;

use config::Config;
use server::WebServer;
use env_logger;

fn main() {
    // Initialize the logger
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();
    
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

    log::info!("Starting server with config from: {}", config_path);
    
    let mut server = WebServer::new(config);
    
    if let Err(e) = server.run() {
        eprintln!("Server error: {}", e);
        process::exit(1);
    }
}