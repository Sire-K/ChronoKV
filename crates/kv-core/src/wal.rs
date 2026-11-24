use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

const OP_SET: u8 = 1;
const OP_DEL: u8 = 2;

fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn compute_record_crc(op: u8, key_bytes: &[u8], val_bytes: &[u8]) -> io::Result<u32> {
    let key_len = u32::try_from(key_bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "key too large"))?;
    let val_len = u32::try_from(val_bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "value too large"))?;

    let mut payload = Vec::with_capacity(1 + 4 + 4 + key_bytes.len() + val_bytes.len());
    payload.push(op);
    payload.extend_from_slice(&key_len.to_le_bytes());
    payload.extend_from_slice(&val_len.to_le_bytes());
    payload.extend_from_slice(key_bytes);
    payload.extend_from_slice(val_bytes);

    Ok(crc32_ieee(&payload))
}

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
        let crc = compute_record_crc(OP_SET, key_bytes, value)?;

        self.writer.write_all(&crc.to_le_bytes())?;
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
        let crc = compute_record_crc(OP_DEL, key_bytes, &[])?;

        self.writer.write_all(&crc.to_le_bytes())?;
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
            let mut crc_buf = [0u8; 4];
            match reader.read_exact(&mut crc_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
            let expected_crc = u32::from_le_bytes(crc_buf);

            let mut op_buf = [0u8; 1];
            if reader.read_exact(&mut op_buf).is_err() {
                break;
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

            let actual_crc = compute_record_crc(op_buf[0], &key_buf, &val_buf)?;
            if actual_crc != expected_crc {
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
