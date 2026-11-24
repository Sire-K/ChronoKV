mod resp;

use kv_core::store::Store;
use resp::{read_command, Command};
use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn execute_command(store: &Arc<Mutex<Store>>, cmd: Command) -> String {
    match cmd {
        Command::Ping => "+PONG\r\n".to_string(),
        Command::Set { key, value } => {
            let mut guard = store.lock().unwrap();
            match guard.set(&key, value.as_bytes()) {
                Ok(()) => "+OK\r\n".to_string(),
                Err(e) => format!("-ERR {e}\r\n"),
            }
        }
        Command::Get { key } => {
            let guard = store.lock().unwrap();
            match guard.get(&key) {
                Some(bytes) => {
                    let text = String::from_utf8_lossy(bytes);
                    format!("+{text}\r\n")
                }
                None => "$-1\r\n".to_string(),
            }
        }
        Command::Del { key } => {
            let mut guard = store.lock().unwrap();
            match guard.del(&key) {
                Ok(true) => ":1\r\n".to_string(),
                Ok(false) => ":0\r\n".to_string(),
                Err(e) => format!("-ERR {e}\r\n"),
            }
        }
    }
}

fn handle_client(stream: TcpStream, store: Arc<Mutex<Store>>) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    while let Some(parsed) = read_command(&mut reader)? {
        let response = match parsed {
            Ok(cmd) => execute_command(&store, cmd),
            Err(msg) => format!("-ERR {msg}\r\n"),
        };
        writer.write_all(response.as_bytes())?;
        writer.flush()?;
    }

    Ok(())
}

fn main() -> io::Result<()> {

    let data_dir = PathBuf::from("/tmp/chronokv");
    std::fs::create_dir_all(&data_dir)?;

    let wal_path = data_dir.join("chronokv.wal");
    let pages_path = data_dir.join("chronokv.pages");
    let store = Arc::new(Mutex::new(Store::open(&wal_path, &pages_path)?));

    let addr = "0.0.0.0:6379";
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
