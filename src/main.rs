use std::env;
use std::process;

mod config;
mod server;
mod http;
mod cgi;
mod utils;
mod static_handler;
mod upload_handler;

use config::Config;
use server::WebServer;
use env_logger;

fn main() {
    // Ensure uploads directory exists at startup
    if let Err(e) = std::fs::create_dir_all("./www/uploads") {
        eprintln!("Failed to create ./www/uploads: {}", e);
    }
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
    
    let mut server = match WebServer::new(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to initialize server: {}", e);
            std::process::exit(1);
        }
    };

    // Catch panics in the main event loop
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Err(e) = server.run() {
            eprintln!("Server error: {}", e);
        }
    }));
    if let Err(panic_info) = result {
        eprintln!("Server panicked: {:?}", panic_info);
        // Optionally: restart the server or just continue
    }
}