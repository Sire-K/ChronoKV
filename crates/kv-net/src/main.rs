use kv_core::store::Store;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn handle_line(store: &Arc<Mutex<Store>>, line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut parts = trimmed.splitn(3, ' ');
    let cmd = parts.next().unwrap_or("").to_ascii_uppercase();

    match cmd.as_str() {
        "PING" => "+PONG\r\n".to_string(),
        "SET" => {
            let Some(key) = parts.next() else {
                return "-ERR wrong number of arguments for 'set'\r\n".to_string();
            };
            let Some(val) = parts.next() else {
                return "-ERR wrong number of arguments for 'set'\r\n".to_string();
            };

            let mut guard = store.lock().unwrap();
            match guard.set(key, val.as_bytes()) {
                Ok(()) => "+OK\r\n".to_string(),
                Err(e) => format!("-ERR {e}\r\n"),
            }
        }
        "GET" => {
            let Some(key) = parts.next() else {
                return "-ERR wrong number of arguments for 'get'\r\n".to_string();
            };

            let guard = store.lock().unwrap();
            match guard.get(key) {
                Some(bytes) => {
                    let text = String::from_utf8_lossy(bytes);
                    format!("+{text}\r\n")
                }
                None => "$-1\r\n".to_string(),
            }
        }
        "DEL" => {
            let Some(key) = parts.next() else {
                return "-ERR wrong number of arguments for 'del'\r\n".to_string();
            };

            let mut guard = store.lock().unwrap();
            match guard.del(key) {
                Ok(true) => ":1\r\n".to_string(),
                Ok(false) => ":0\r\n".to_string(),
                Err(e) => format!("-ERR {e}\r\n"),
            }
        }
        _ => format!("-ERR unknown command '{cmd}'\r\n"),
    }
}

fn handle_client(stream: TcpStream, store: Arc<Mutex<Store>>) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        let response = handle_line(&store, &line);
        if !response.is_empty() {
            writer.write_all(response.as_bytes())?;
            writer.flush()?;
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let data_dir = PathBuf::from("/tmp/chronokv");
    std::fs::create_dir_all(&data_dir)?;

    let wal_path = data_dir.join("chronokv.wal");
    let pages_path = data_dir.join("chronokv.pages");
    let store = Arc::new(Mutex::new(Store::open(&wal_path, &pages_path)?));

    let addr = "127.0.0.1:6379";
    let listener = TcpListener::bind(addr)?;
    println!("ChronoKV listening on {addr}");

    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let store_clone = Arc::clone(&store);
                std::thread::spawn(move || {
                    if let Err(e) = handle_client(stream, store_clone) {
                        eprintln!("client error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }

    Ok(())
}
