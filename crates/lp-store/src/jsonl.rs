//! Crash-safe file primitives: `<crc32 hex>\t<json>\n` ledger lines and temp+fsync+rename writes.
use lp_core::ports::{StoreError, StoreResult};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
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

/// Valid lines in order. A corrupt interior line is skipped, never allowed to hide later records.
fn valid_lines(data: &[u8]) -> Vec<&str> {
    data.split_inclusive(|&b| b == b'\n')
        .filter_map(|raw| raw.strip_suffix(b"\n").and_then(decode_line))
        .collect()
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
    valid_lines(&data)
        .into_iter()
        .map(|l| serde_json::from_str(l).map_err(be))
        .collect()
}

/// Create `dir` and missing parents owner-only (the tree holds chats and coding transcripts).
pub(crate) fn create_dir(dir: &Path) -> StoreResult<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(be)
}

fn create_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
}

/// Exclusive cross-process lock on a sidecar `.<name>.lock`, released when the guard drops.
pub(crate) fn lock(path: &Path) -> StoreResult<File> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| be("bad file name"))?;
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(path.with_file_name(format!(".{name}.lock")))
        .map_err(be)?;
    f.lock().map_err(be)?;
    Ok(f)
}

/// Drop a torn or corrupt final line (left by a crash), reading only the tail. Returns the new length.
fn trim_tail(f: &mut File) -> StoreResult<u64> {
    let len = f.metadata().map_err(be)?.len();
    let (mut pos, mut tail) = (len, vec![]);
    let start = loop {
        let step = pos.min(8192);
        pos -= step;
        let mut chunk = vec![0; step as usize];
        f.seek(SeekFrom::Start(pos)).map_err(be)?;
        f.read_exact(&mut chunk).map_err(be)?;
        chunk.extend(tail);
        tail = chunk;
        let body = tail.strip_suffix(b"\n").unwrap_or(&tail);
        if let Some(i) = body.iter().rposition(|&b| b == b'\n') {
            break pos + i as u64 + 1;
        }
        if pos == 0 {
            break 0;
        }
    };
    let line = &tail[(start - pos) as usize..];
    let ok = line.strip_suffix(b"\n").and_then(decode_line).is_some();
    if len == 0 || ok {
        return Ok(len);
    }
    f.set_len(start).map_err(be)?;
    Ok(start)
}

/// Append one line, first truncating a corrupt final line left by a crash.
pub(crate) fn append(path: &Path, value: &impl Serialize) -> StoreResult<()> {
    let line = encode(value)?;
    let _lock = lock(path)?;
    let existed = path.exists();
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(be)?;
    let valid = trim_tail(&mut f)?;
    f.seek(SeekFrom::Start(valid)).map_err(be)?;
    f.write_all(&line).map_err(be)?;
    f.sync_all().map_err(be)?;
    if !existed {
        sync_dir(path)?;
    }
    Ok(())
}

fn sync_dir(path: &Path) -> StoreResult<()> {
    let dir = path.parent().ok_or_else(|| be("path has no parent"))?;
    File::open(dir).and_then(|d| d.sync_all()).map_err(be)
}

/// Replace the whole file atomically: pid-named temp file in the same dir, fsync, rename, fsync dir.
/// Callers hold `lock(path)`.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> StoreResult<()> {
    let dir = path.parent().ok_or_else(|| be("path has no parent"))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| be("bad file name"))?;
    let tmp = dir.join(format!(".{name}.tmp.{}", std::process::id()));
    let mut f = create_file(&tmp).map_err(be)?;
    f.write_all(bytes).map_err(be)?;
    f.sync_all().map_err(be)?;
    fs::rename(&tmp, path).map_err(be)?;
    sync_dir(path)
}

pub(crate) fn rewrite<T: Serialize>(path: &Path, items: &[T]) -> StoreResult<()> {
    let mut buf = vec![];
    for i in items {
        buf.extend(encode(i)?);
    }
    let _lock = lock(path)?;
    write_atomic(path, &buf)
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> StoreResult<()> {
    let _lock = lock(path)?;
    write_atomic(path, &serde_json::to_vec(value).map_err(be)?)
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path, id: &str) -> StoreResult<T> {
    match fs::read(path) {
        Ok(d) => serde_json::from_slice(&d).map_err(be),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(StoreError::NotFound(id.into())),
        Err(e) => Err(be(e)),
    }
}
