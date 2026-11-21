//! A tiny ZIP writer for the exported Cargo project.
//!
//! Only the `store` method (no compression) is used: the generated sources are
//! small, and this keeps the plugin `no_std` + `alloc` with no dependencies.

use alloc::string::String;
use alloc::vec::Vec;

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

/// A file to put in the archive.
pub struct Entry {
    pub path: String,
    pub data: Vec<u8>,
}

impl Entry {
    pub fn new(path: &str, data: Vec<u8>) -> Self {
        Self { path: path.to_string(), data }
    }
}

struct Record {
    crc: u32,
    size: u32,
    offset: u32,
}

fn u16le(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn u32le(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// CRC-32 (IEEE, reflected), the checksum ZIP uses.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

/// Build a ZIP archive from `entries` (paths use `/` separators).
pub fn archive(entries: &[Entry]) -> Result<Vec<u8>, String> {
    if entries.is_empty() {
        return Err("nothing to archive".into());
    }
    let mut out = Vec::new();
    let mut records = Vec::with_capacity(entries.len());
    // A fixed timestamp keeps the export deterministic.
    let (time, date) = (0u16, (37u16 << 9) | (1u16 << 5) | 1u16); // 2026-01-01 00:00

    for entry in entries {
        let offset = u32::try_from(out.len()).map_err(|_| "archive is too large".to_string())?;
        let size = u32::try_from(entry.data.len()).map_err(|_| "a file is too large".to_string())?;
        let name = entry.path.as_bytes();
        let name_len = u16::try_from(name.len()).map_err(|_| "a path is too long".to_string())?;
        let crc = crc32(&entry.data);

        u32le(&mut out, LOCAL);
        u16le(&mut out, 20); // version needed
        u16le(&mut out, 0); // flags
        u16le(&mut out, 0); // stored
        u16le(&mut out, time);
        u16le(&mut out, date);
        u32le(&mut out, crc);
        u32le(&mut out, size);
        u32le(&mut out, size);
        u16le(&mut out, name_len);
        u16le(&mut out, 0); // extra length
        out.extend_from_slice(name);
        out.extend_from_slice(&entry.data);

        records.push(Record { crc, size, offset });
    }

    let central_start = u32::try_from(out.len()).map_err(|_| "archive is too large".to_string())?;
    for (entry, record) in entries.iter().zip(records.iter()) {
        let name = entry.path.as_bytes();
        let name_len = u16::try_from(name.len()).map_err(|_| "a path is too long".to_string())?;
        u32le(&mut out, CENTRAL);
        u16le(&mut out, 20); // made by MS-DOS
        u16le(&mut out, 20); // needed
        u16le(&mut out, 0); // flags
        u16le(&mut out, 0); // stored
        u16le(&mut out, time);
        u16le(&mut out, date);
        u32le(&mut out, record.crc);
        u32le(&mut out, record.size);
        u32le(&mut out, record.size);
        u16le(&mut out, name_len);
        u16le(&mut out, 0); // extra
        u16le(&mut out, 0); // comment
        u16le(&mut out, 0); // disk start
        u16le(&mut out, 0); // internal attributes
        u32le(&mut out, 0); // external attributes
        u32le(&mut out, record.offset);
        out.extend_from_slice(name);
    }
    let central_size = u32::try_from(out.len()).map_err(|_| "archive is too large".to_string())? - central_start;
    let count = u16::try_from(entries.len()).map_err(|_| "too many files".to_string())?;

    u32le(&mut out, END);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, count);
    u16le(&mut out, count);
    u32le(&mut out, central_size);
    u32le(&mut out, central_start);
    u16le(&mut out, 0); // comment length
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn archive_has_headers_and_entries() {
        let bytes = archive(&[
            Entry::new("Cargo.toml", b"[package]\n".to_vec()),
            Entry::new("src/main.rs", b"fn main() {}\n".to_vec()),
        ])
        .unwrap_or_default();
        assert_eq!(&bytes[0..4], &LOCAL.to_le_bytes());
        assert!(bytes.windows(4).any(|w| w == CENTRAL.to_le_bytes()));
        assert!(bytes.windows(4).any(|w| w == END.to_le_bytes()));
        assert!(bytes.windows(11).any(|w| w == b"src/main.rs"));
    }
}
