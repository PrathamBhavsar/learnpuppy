//! Crash-safe file primitives: `<crc32 hex>\t<json>\n` ledger lines and temp+fsync+rename writes.
use lp_core::ports::{StoreError, StoreResult};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

pub(crate) fn be(e: impl std::fmt::Display) -> StoreError {
    StoreError::Backend(e.to_string())
}

/// Reject ids that could escape their directory.
pub(crate) fn check_id(id: &str) -> StoreResult<()> {
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return Err(StoreError::Backend(format!("invalid id: {id:?}")));
    }
    Ok(())
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 1 { 0xEDB8_8320 } else { 0 };
        }
    }
    !crc
}

fn encode(value: &impl Serialize) -> StoreResult<Vec<u8>> {
    let json = serde_json::to_string(value).map_err(be)?;
    Ok(format!("{:08x}\t{json}\n", crc32(json.as_bytes())).into_bytes())
}

fn decode_line(line: &[u8]) -> Option<&str> {
    let s = std::str::from_utf8(line).ok()?;
    let (crc, json) = s.split_once('\t')?;
    (u32::from_str_radix(crc, 16).ok()? == crc32(json.as_bytes())).then_some(json)
}

/// Valid leading lines and their byte length; a torn or corrupt line ends the ledger.
fn valid_prefix(data: &[u8]) -> (Vec<&str>, usize) {
    let (mut lines, mut len) = (vec![], 0);
    for raw in data.split_inclusive(|&b| b == b'\n') {
        let Some(line) = raw.strip_suffix(b"\n").and_then(decode_line) else {
            break;
        };
        lines.push(line);
        len += raw.len();
    }
    (lines, len)
}

fn read_bytes(path: &Path) -> StoreResult<Vec<u8>> {
    match fs::read(path) {
        Ok(d) => Ok(d),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(be(e)),
    }
}

pub(crate) fn read_all<T: DeserializeOwned>(path: &Path) -> StoreResult<Vec<T>> {
    let data = read_bytes(path)?;
    valid_prefix(&data)
        .0
        .into_iter()
        .map(|l| serde_json::from_str(l).map_err(be))
        .collect()
}

/// Append one line, first truncating any corrupt tail left by a crash.
pub(crate) fn append(path: &Path, value: &impl Serialize) -> StoreResult<()> {
    let line = encode(value)?;
    let valid = valid_prefix(&read_bytes(path)?).1 as u64;
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(be)?;
    f.set_len(valid).map_err(be)?;
    f.seek(SeekFrom::Start(valid)).map_err(be)?;
    f.write_all(&line).map_err(be)?;
    f.sync_all().map_err(be)
}

/// Replace the whole file atomically: temp file in the same dir, fsync, rename, fsync dir.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> StoreResult<()> {
    let dir = path.parent().ok_or_else(|| be("path has no parent"))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| be("bad file name"))?;
    let tmp = dir.join(format!(".{name}.tmp"));
    let mut f = File::create(&tmp).map_err(be)?;
    f.write_all(bytes).map_err(be)?;
    f.sync_all().map_err(be)?;
    fs::rename(&tmp, path).map_err(be)?;
    File::open(dir).and_then(|d| d.sync_all()).map_err(be)
}

pub(crate) fn rewrite<T: Serialize>(path: &Path, items: &[T]) -> StoreResult<()> {
    let mut buf = vec![];
    for i in items {
        buf.extend(encode(i)?);
    }
    write_atomic(path, &buf)
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> StoreResult<()> {
    write_atomic(path, &serde_json::to_vec(value).map_err(be)?)
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path, id: &str) -> StoreResult<T> {
    match fs::read(path) {
        Ok(d) => serde_json::from_slice(&d).map_err(be),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(StoreError::NotFound(id.into())),
        Err(e) => Err(be(e)),
    }
}
