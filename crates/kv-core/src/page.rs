use std::io;

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_HEADER_SIZE: usize = 16;
pub const PAGE_PAYLOAD_SIZE: usize = PAGE_SIZE - PAGE_HEADER_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageHeader {
    pub page_id: u64,
    pub payload_len: u32,
    pub flags: u32,
}

impl PageHeader {
    pub fn to_bytes(self) -> [u8; PAGE_HEADER_SIZE] {
        let mut buf = [0u8; PAGE_HEADER_SIZE];
        buf[0..8].copy_from_slice(&self.page_id.to_le_bytes());
        buf[8..12].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[12..16].copy_from_slice(&self.flags.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8; PAGE_HEADER_SIZE]) -> Self {
        Self {
            page_id: u64::from_le_bytes(buf[0..8].try_into().unwrap()),
            payload_len: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
            flags: u32::from_le_bytes(buf[12..16].try_into().unwrap()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub header: PageHeader,
    pub data: [u8; PAGE_PAYLOAD_SIZE],
}

impl Page {
    pub fn new(page_id: u64, payload: &[u8], flags: u32) -> io::Result<Self> {
        if payload.len() > PAGE_PAYLOAD_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "payload exceeds 4KB page capacity",
            ));
        }

        let mut data = [0u8; PAGE_PAYLOAD_SIZE];
        data[..payload.len()].copy_from_slice(payload);

        Ok(Self {
            header: PageHeader {
                page_id,
                payload_len: payload.len() as u32,
                flags,
            },
            data,
        })
    }

    pub fn to_bytes(&self) -> [u8; PAGE_SIZE] {
        let mut out = [0u8; PAGE_SIZE];
        out[0..PAGE_HEADER_SIZE].copy_from_slice(&self.header.to_bytes());
        out[PAGE_HEADER_SIZE..PAGE_SIZE].copy_from_slice(&self.data);
        out
    }

    pub fn from_bytes(raw: &[u8; PAGE_SIZE]) -> io::Result<Self> {
        let hdr_bytes: [u8; PAGE_HEADER_SIZE] = raw[0..PAGE_HEADER_SIZE].try_into().unwrap();
        let header = PageHeader::from_bytes(&hdr_bytes);

        let payload_len = header.payload_len as usize;
        if payload_len > PAGE_PAYLOAD_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "corrupt page header: payload_len larger than page",
            ));
        }

        let mut data = [0u8; PAGE_PAYLOAD_SIZE];
        data.copy_from_slice(&raw[PAGE_HEADER_SIZE..PAGE_SIZE]);

        Ok(Self { header, data })
    }

    pub fn payload(&self) -> &[u8] {
        let len = (self.header.payload_len as usize).min(PAGE_PAYLOAD_SIZE);
        &self.data[..len]
    }
}
