use std::collections::HashMap;
use std::sync::Mutex;
use rand::Rng;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SESSION_STORE: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

pub fn get_or_create_session_id(cookie_header: Option<&String>) -> String {
    // Try to find session id in cookie header
    if let Some(cookie_header) = cookie_header {
        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            if let Some((name, value)) = cookie.split_once('=') {
                if name == "SESSIONID" {
                    return value.to_string();
                }
            }
        }
    }
    // Not found, generate a new one
    let mut rng = rand::thread_rng();
    let session_id: String = (0..16).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect();
    // Store the session (for demonstration, value is empty string)
    let mut store = SESSION_STORE.lock().unwrap();
    store.insert(session_id.clone(), String::new());
    session_id
}
