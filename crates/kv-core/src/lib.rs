pub mod page;
pub mod store;
pub mod wal;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::{Page, PAGE_PAYLOAD_SIZE, PAGE_SIZE};
    use crate::store::Store;
    use crate::wal::Wal;
    use std::io;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn write_and_replay() -> io::Result<()> {
        let path = "/tmp/test_chronokv.wal";
        let _ = std::fs::remove_file(path);

        let mut wal = Wal::open(path)?;
        wal.append_set("user:1", b"Abhishekh")?;
        wal.append_set("user:2", b"Amir")?;
        wal.append_del("user:1")?;

        let state = Wal::replay(path)?;
        assert_eq!(state.get("user:1"), None);
        assert_eq!(
            state.get("user:2").map(Vec::as_slice),
            Some(b"Amir".as_slice())
        );
        Ok(())
    }

    #[test]
    fn page_roundtrip_and_size() -> io::Result<()> {
        let page = Page::new(42, b"hello 4kb page", 1)?;
        let raw = page.to_bytes();
        assert_eq!(raw.len(), PAGE_SIZE);

        let decoded = Page::from_bytes(&raw)?;
        assert_eq!(decoded, page);
        assert_eq!(decoded.payload(), b"hello 4kb page");
        Ok(())
    }

    #[test]
    fn page_rejects_oversized_payload_and_corrupt_header() {
        let too_big = vec![b'x'; PAGE_PAYLOAD_SIZE + 1];
        assert!(Page::new(1, &too_big, 0).is_err());

        let page = Page::new(7, b"sample", 0).unwrap();
        let mut raw = page.to_bytes();
        // Force payload_len (bytes 8..12) to a value larger than PAGE_PAYLOAD_SIZE.
        raw[8..12].copy_from_slice(&(PAGE_SIZE as u32).to_le_bytes());

        assert!(Page::from_bytes(&raw).is_err());
    }

    #[test]
    fn store_wal_recovery_and_page_snapshots() -> io::Result<()> {
        let wal_path = "/tmp/test_store_chronokv.wal";
        let pages_path = "/tmp/test_store_chronokv.pages";
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(pages_path);

        {
            let mut store = Store::open(wal_path, pages_path)?;
            store.set("k1", b"v1")?;
            store.set("k2", b"v2")?;
            assert_eq!(store.del("k1")?, true);
            assert_eq!(store.get("k1"), None);
            assert_eq!(store.get("k2"), Some(b"v2".as_slice()));

            let written_pages = store.write_snapshot_pages()?;
            assert!(written_pages >= 1);
        }

        // Reopen without touching WAL manually—state must come back from WAL replay.
        let mut reopened = Store::open(wal_path, pages_path)?;
        assert_eq!(reopened.get("k1"), None);
        assert_eq!(reopened.get("k2"), Some(b"v2".as_slice()));

        let pages = reopened.read_snapshot_pages()?;
        assert!(!pages.is_empty());
        assert_eq!(pages[0].header.page_id, 0);

        let pages_file_len = std::fs::metadata(pages_path)?.len();
        assert_eq!(pages_file_len % (PAGE_SIZE as u64), 0);

        Ok(())
    }
}
