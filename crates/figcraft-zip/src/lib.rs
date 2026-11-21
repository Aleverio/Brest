//! Deterministic, store-only ZIP writer.
//!
//! Generated projects are text and small images, so compression is not worth
//! the extra code. A fixed timestamp and sorted entry order make exports
//! byte-for-byte reproducible.

#![cfg_attr(test, allow(clippy::expect_used))]

use std::fmt;

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

/// `save_file` in the FigCraft SDK refuses payloads above 16 MiB.
pub const MAX_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum ZipError {
    Empty,
    TooLarge { bytes: usize, limit: usize },
    PathTooLong(String),
    Overflow,
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "nothing to archive"),
            Self::TooLarge { bytes, limit } => {
                write!(f, "archive would be {bytes} bytes, limit is {limit}")
            }
            Self::PathTooLong(path) => write!(f, "path is too long for ZIP: {path}"),
            Self::Overflow => write!(f, "archive offsets overflowed 32 bits"),
        }
    }
}

impl std::error::Error for ZipError {}

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

fn u16le(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn u32le(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Build a ZIP from `(path, bytes)` pairs. Paths use `/` separators and are
/// sorted before writing, so the result does not depend on input order.
pub fn archive<'a, I>(entries: I) -> Result<Vec<u8>, ZipError>
where
    I: IntoIterator<Item = (&'a str, &'a [u8])>,
{
    let mut entries: Vec<(&str, &[u8])> = entries.into_iter().collect();
    if entries.is_empty() {
        return Err(ZipError::Empty);
    }
    entries.sort_by(|left, right| left.0.cmp(right.0));

    let mut out = Vec::new();
    let mut records = Vec::with_capacity(entries.len());
    // Fixed timestamp: 2026-01-01 00:00.
    let (time, date) = (0u16, (37u16 << 9) | (1u16 << 5) | 1u16);

    for (path, data) in &entries {
        let offset = u32::try_from(out.len()).map_err(|_| ZipError::Overflow)?;
        let size = u32::try_from(data.len()).map_err(|_| ZipError::Overflow)?;
        let name = path.as_bytes();
        let name_len = u16::try_from(name.len()).map_err(|_| ZipError::PathTooLong((*path).into()))?;
        let crc = crc32(data);

        u32le(&mut out, LOCAL);
        u16le(&mut out, 20);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u16le(&mut out, time);
        u16le(&mut out, date);
        u32le(&mut out, crc);
        u32le(&mut out, size);
        u32le(&mut out, size);
        u16le(&mut out, name_len);
        u16le(&mut out, 0);
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        records.push((crc, size, offset));
    }

    let central_start = u32::try_from(out.len()).map_err(|_| ZipError::Overflow)?;
    for ((path, _), (crc, size, offset)) in entries.iter().zip(records.iter()) {
        let name = path.as_bytes();
        let name_len = u16::try_from(name.len()).map_err(|_| ZipError::PathTooLong((*path).into()))?;
        u32le(&mut out, CENTRAL);
        u16le(&mut out, 20);
        u16le(&mut out, 20);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u16le(&mut out, time);
        u16le(&mut out, date);
        u32le(&mut out, *crc);
        u32le(&mut out, *size);
        u32le(&mut out, *size);
        u16le(&mut out, name_len);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u32le(&mut out, 0);
        u32le(&mut out, *offset);
        out.extend_from_slice(name);
    }

    let central_size = u32::try_from(out.len()).map_err(|_| ZipError::Overflow)? - central_start;
    let count = u16::try_from(entries.len()).map_err(|_| ZipError::Overflow)?;
    u32le(&mut out, END);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, count);
    u16le(&mut out, count);
    u32le(&mut out, central_size);
    u32le(&mut out, central_start);
    u16le(&mut out, 0);

    if out.len() > MAX_ARCHIVE_BYTES {
        return Err(ZipError::TooLarge {
            bytes: out.len(),
            limit: MAX_ARCHIVE_BYTES,
        });
    }
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
    fn archive_is_sorted_and_deterministic() {
        let first = archive([("b.txt", &b"b"[..]), ("a.txt", &b"a"[..])]).expect("archive");
        let second = archive([("a.txt", &b"a"[..]), ("b.txt", &b"b"[..])]).expect("archive");
        assert_eq!(first, second);
        let a = first.windows(5).position(|window| window == b"a.txt").expect("a.txt");
        let b = first.windows(5).position(|window| window == b"b.txt").expect("b.txt");
        assert!(a < b);
        assert_eq!(&first[0..4], &LOCAL.to_le_bytes());
        assert!(first.windows(4).any(|window| window == CENTRAL.to_le_bytes()));
        assert!(first.windows(4).any(|window| window == END.to_le_bytes()));
    }

    #[test]
    fn empty_archive_is_rejected() {
        assert_eq!(archive(std::iter::empty()), Err(ZipError::Empty));
    }
}
