#![allow(dead_code)]

use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
pub fn var(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    while v >= 128 {
        out.push(v as u8 | 128);
        v >>= 7;
    }
    out.push(v as u8);
    out
}
pub fn int(n: u64, v: u64) -> Vec<u8> {
    [var(n << 3), var(v)].concat()
}
pub fn msg(n: u64, bytes: &[u8]) -> Vec<u8> {
    [var(n << 3 | 2), var(bytes.len() as u64), bytes.to_vec()].concat()
}
pub fn zz(v: i64) -> u64 {
    ((v as u64) << 1) ^ ((v >> 63) as u64)
}
pub fn packed(n: u64, values: &[u64]) -> Vec<u8> {
    msg(n, &values.iter().flat_map(|v| var(*v)).collect::<Vec<_>>())
}
pub fn block(kind: &[u8], raw: &[u8]) -> Vec<u8> {
    blob(kind, &msg(1, raw))
}
pub fn blob(kind: &[u8], blob: &[u8]) -> Vec<u8> {
    let header = [msg(1, kind), int(3, blob.len() as u64)].concat();
    [
        (header.len() as u32).to_be_bytes().to_vec(),
        header,
        blob.to_vec(),
    ]
    .concat()
}
pub fn header() -> Vec<u8> {
    block(
        b"OSMHeader",
        &[msg(4, b"OsmSchema-V0.6"), msg(5, b"LocationsOnWays")].concat(),
    )
}
pub fn table() -> Vec<u8> {
    msg(1, &[msg(1, b""), msg(1, b"key"), msg(1, b"value")].concat())
}
pub struct Temp(pub PathBuf);
impl Temp {
    pub fn bytes(bytes: &[u8], extension: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "inputosm-test-{}-{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            extension
        ));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
    pub fn pbf(raw: &[u8]) -> Self {
        Self::bytes(&[header(), block(b"OSMData", raw)].concat(), "pbf")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub fn stored_zlib(raw: &[u8]) -> Vec<u8> {
    let mut result = vec![0x78, 0x01];
    let chunks = raw.len().max(1).div_ceil(65535);
    for index in 0..chunks {
        let start = index * 65535;
        let end = raw.len().min(start + 65535);
        let length = (end - start) as u16;
        result.push(u8::from(index + 1 == chunks));
        result.extend(length.to_le_bytes());
        result.extend((!length).to_le_bytes());
        result.extend(&raw[start..end]);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    result.extend(((b << 16) | a).to_be_bytes());
    result
}
pub fn compressed(kind: &[u8], raw: &[u8]) -> Vec<u8> {
    blob(
        kind,
        &[int(2, raw.len() as u64), msg(3, &stored_zlib(raw))].concat(),
    )
}
