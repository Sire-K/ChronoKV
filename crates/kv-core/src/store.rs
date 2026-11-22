use crate::page::{Page, PAGE_PAYLOAD_SIZE, PAGE_SIZE};
use crate::wal::Wal;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct Store {
    wal: Wal,
    pages_file: File,
    data: HashMap<String, Vec<u8>>,
    next_page_id: u64,
}

impl Store {
    pub fn open<P: AsRef<Path>>(wal_path: P, pages_path: P) -> io::Result<Self> {
        let data = Wal::replay(&wal_path)?;
        let wal = Wal::open(&wal_path)?;

        let mut pages_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&pages_path)?;

        let len = pages_file.metadata()?.len();
        let next_page_id = len / (PAGE_SIZE as u64);
        pages_file.seek(SeekFrom::End(0))?;

        Ok(Self {
            wal,
            pages_file,
            data,
            next_page_id,
        })
    }

    pub fn set(&mut self, key: &str, value: &[u8]) -> io::Result<()> {
        self.wal.append_set(key, value)?;
        self.data.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.data.get(key).map(Vec::as_slice)
    }

    pub fn del(&mut self, key: &str) -> io::Result<bool> {
        self.wal.append_del(key)?;
        Ok(self.data.remove(key).is_some())
    }

    /// Snapshots current in-memory values into 4KB pages on disk.
    pub fn write_snapshot_pages(&mut self) -> io::Result<u64> {
        self.pages_file.set_len(0)?;
        self.pages_file.seek(SeekFrom::Start(0))?;
        self.next_page_id = 0;

        for (k, v) in &self.data {
            let mut entry = Vec::with_capacity(4 + k.len() + 4 + v.len());
            let k_len = u32::try_from(k.len())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "key too large"))?;
            let v_len = u32::try_from(v.len())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "value too large"))?;

            entry.extend_from_slice(&k_len.to_le_bytes());
            entry.extend_from_slice(k.as_bytes());
            entry.extend_from_slice(&v_len.to_le_bytes());
            entry.extend_from_slice(v);

            if entry.len() > PAGE_PAYLOAD_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "entry exceeds single 4KB page payload",
                ));
            }

            let page = Page::new(self.next_page_id, &entry, 1)?;
            self.pages_file.write_all(&page.to_bytes())?;
            self.next_page_id += 1;
        }

        self.pages_file.flush()?;
        Ok(self.next_page_id)
    }

    /// Reads back all 4KB pages from the snapshot file and validates framing.
    pub fn read_snapshot_pages(&mut self) -> io::Result<Vec<Page>> {
        self.pages_file.seek(SeekFrom::Start(0))?;
        let mut pages = Vec::new();

        loop {
            let mut raw = [0u8; PAGE_SIZE];
            match self.pages_file.read_exact(&mut raw) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
            pages.push(Page::from_bytes(&raw)?);
        }

        Ok(pages)
    }
}
