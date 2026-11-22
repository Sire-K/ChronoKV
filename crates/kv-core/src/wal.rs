use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

const OP_SET: u8 = 1;
const OP_DEL: u8 = 2;

pub struct Wal {
    writer: BufWriter<File>,
}

impl Wal {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn append_set(&mut self, key: &str, value: &[u8]) -> io::Result<()> {
        let key_bytes = key.as_bytes();
        let key_len = u32::try_from(key_bytes.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "key too large"))?;
        let val_len = u32::try_from(value.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "value too large"))?;

        self.writer.write_all(&[OP_SET])?;
        self.writer.write_all(&key_len.to_le_bytes())?;
        self.writer.write_all(&val_len.to_le_bytes())?;
        self.writer.write_all(key_bytes)?;
        self.writer.write_all(value)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn append_del(&mut self, key: &str) -> io::Result<()> {
        let key_bytes = key.as_bytes();
        let key_len = u32::try_from(key_bytes.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "key too large"))?;

        self.writer.write_all(&[OP_DEL])?;
        self.writer.write_all(&key_len.to_le_bytes())?;
        self.writer.write_all(&0u32.to_le_bytes())?;
        self.writer.write_all(key_bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn replay<P: AsRef<Path>>(path: P) -> io::Result<HashMap<String, Vec<u8>>> {
        let mut map = HashMap::new();
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(map),
            Err(e) => return Err(e),
        };

        let mut reader = BufReader::new(file);
        loop {
            let mut op_buf = [0u8; 1];
            match reader.read_exact(&mut op_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }

            let mut len_buf = [0u8; 8];
            if reader.read_exact(&mut len_buf).is_err() {
                break;
            }

            let key_len = u32::from_le_bytes(len_buf[0..4].try_into().unwrap()) as usize;
            let val_len = u32::from_le_bytes(len_buf[4..8].try_into().unwrap()) as usize;

            let mut key_buf = vec![0u8; key_len];
            if reader.read_exact(&mut key_buf).is_err() {
                break;
            }

            let mut val_buf = vec![0u8; val_len];
            if reader.read_exact(&mut val_buf).is_err() {
                break;
            }

            let Ok(key) = String::from_utf8(key_buf) else {
                break;
            };

            match op_buf[0] {
                OP_SET => {
                    map.insert(key, val_buf);
                }
                OP_DEL => {
                    map.remove(&key);
                }
                _ => break,
            }
        }

        Ok(map)
    }
}
