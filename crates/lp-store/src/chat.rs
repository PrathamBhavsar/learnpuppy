//! `FsChatStore`: one atomically-replaced `chats/<id>.json` per chat.
use crate::jsonl::{be, check_id, create_dir, lock, read_json, write_atomic, write_json};
use lp_core::fsrs::TurnUsage;
use lp_core::ports::{ChatRecord, ChatStore, ChatSummary, StoreResult};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct FsChatStore {
    dir: PathBuf,
    /// Serialises read-modify-write appends.
    lock: Mutex<()>,
}

impl FsChatStore {
    pub fn open(dir: impl Into<PathBuf>) -> StoreResult<Self> {
        let dir = dir.into();
        create_dir(&dir)?;
        Ok(Self {
            dir,
            lock: Mutex::new(()),
        })
    }

    fn path(&self, id: &str) -> StoreResult<PathBuf> {
        check_id(id)?;
        Ok(self.dir.join(format!("{id}.json")))
    }

    fn modify(&self, id: &str, f: impl FnOnce(&mut ChatRecord)) -> StoreResult<()> {
        let _guard = self.lock.lock().map_err(be)?;
        let path = self.path(id)?;
        let _file_lock = lock(&path)?;
        let mut chat = self.load_chat(id)?;
        f(&mut chat);
        write_atomic(&path, &serde_json::to_vec(&chat).map_err(be)?)
    }
}

impl ChatStore for FsChatStore {
    fn load_chat(&self, id: &str) -> StoreResult<ChatRecord> {
        read_json(&self.path(id)?, id)
    }
    fn save_chat(&self, chat: &ChatRecord) -> StoreResult<()> {
        let _guard = self.lock.lock().map_err(be)?;
        write_json(&self.path(&chat.id)?, chat)
    }
    fn append_turn_usage(&self, id: &str, usage: TurnUsage) -> StoreResult<()> {
        self.modify(id, |c| c.turns.push(usage))
    }
    fn append_extraction_usage(&self, id: &str, usage: TurnUsage) -> StoreResult<()> {
        self.modify(id, |c| c.extractions.push(usage))
    }
    fn list_chats(&self) -> StoreResult<Vec<ChatSummary>> {
        let mut out = vec![];
        for entry in std::fs::read_dir(&self.dir).map_err(be)? {
            let path = entry.map_err(be)?.path();
            if path.extension().is_some_and(|e| e == "json") {
                // One unreadable chat must not hide the rest.
                let Ok(c) = read_json::<ChatRecord>(&path, "") else {
                    continue;
                };
                out.push(ChatSummary {
                    id: c.id,
                    title: c.title,
                    updated_at: c.updated_at,
                });
            }
        }
        out.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(out)
    }
}
