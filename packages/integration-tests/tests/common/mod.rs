#![allow(dead_code)]

use std::env;
use std::net::TcpStream;
use rusqlite::Connection;


pub fn get_server_url() -> String {
    env::var("SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

pub fn get_ws_url() -> String {
    env::var("WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:8080/ws".to_string())
}

pub fn is_server_running() -> bool {
    let url = get_server_url();
    let addr = url.trim_start_matches("http://").trim_start_matches("https://");
    TcpStream::connect(addr).is_ok()
}

pub fn create_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
    conn
}
