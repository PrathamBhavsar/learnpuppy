//! `FsTextbookStore`: `<root>/textbooks/<bookId>/{source.pdf,textbook.jsonl}` (U7).
//!
//! Lines use the crash-safe `jsonl` format. Every method checks the book id first, so an id can
//! never name anything outside `textbooks/`.
use crate::jsonl::{append, be, create_dir, lock, read_all, write_atomic};
use lp_core::ports::{StoreError, StoreResult};
use lp_core::textbook::{valid_book_id, TextbookEvent};
use std::path::PathBuf;

pub struct FsTextbookStore {
    root: PathBuf,
}

fn check_book(id: &str) -> StoreResult<()> {
    if valid_book_id(id) {
        Ok(())
    } else {
        Err(StoreError::Backend(format!("invalid book id {id:?}")))
    }
}

impl FsTextbookStore {
    /// `root` is the workspace root; nothing is created until the first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn dir(&self, book: &str) -> StoreResult<PathBuf> {
        check_book(book)?;
        Ok(self.root.join("textbooks").join(book))
    }

    fn ledger(&self, book: &str) -> StoreResult<PathBuf> {
        Ok(self.dir(book)?.join("textbook.jsonl"))
    }

    pub fn append_event(&self, book: &str, ev: &TextbookEvent) -> StoreResult<()> {
        create_dir(&self.dir(book)?)?;
        append(&self.ledger(book)?, ev)
    }

    /// A missing ledger is an empty book.
    pub fn events(&self, book: &str) -> StoreResult<Vec<TextbookEvent>> {
        read_all(&self.ledger(book)?)
    }

    /// True once the ledger holds an `Imported` line (a bare `source.pdf` does not count).
    pub fn exists(&self, book: &str) -> StoreResult<bool> {
        Ok(self
            .events(book)?
            .iter()
            .any(|e| matches!(e, TextbookEvent::Imported { .. })))
    }

    /// Valid book ids with an `Imported` line, sorted.
    pub fn list(&self) -> StoreResult<Vec<String>> {
        let entries = match std::fs::read_dir(self.root.join("textbooks")) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(be(e)),
        };
        let mut out = vec![];
        for entry in entries.flatten() {
            let Some(id) = entry.file_name().to_str().map(String::from) else {
                continue;
            };
            if valid_book_id(&id) && self.exists(&id).unwrap_or(false) {
                out.push(id);
            }
        }
        out.sort();
        Ok(out)
    }

    /// Written once per upload, mode 0600, atomically under the ledger lock.
    pub fn save_source(&self, book: &str, pdf: &[u8]) -> StoreResult<()> {
        let dir = self.dir(book)?;
        create_dir(&dir)?;
        let path = dir.join("source.pdf");
        let _lock = lock(&path)?;
        write_atomic(&path, pdf)
    }

    pub fn source_path(&self, book: &str) -> StoreResult<PathBuf> {
        Ok(self.dir(book)?.join("source.pdf"))
    }

    /// Removes the whole book directory (a failed upload leaves nothing behind).
    pub fn remove(&self, book: &str) -> StoreResult<()> {
        match std::fs::remove_dir_all(self.dir(book)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(be(e)),
        }
    }
}
