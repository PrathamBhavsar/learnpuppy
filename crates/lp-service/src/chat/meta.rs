//! Server-side facts about a chat that the agent `Session` does not model: when each message was
//! added, which assistant messages a cancel cut short, and the `clientId`s already applied.
//!
//! Stored as `lpMeta` next to the session fields in `ChatRecord.body`. `Session` ignores unknown
//! fields when it deserializes, so the agent crates never see it.

use lp_agent::loops::Session;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const KEY: &str = "lpMeta";

/// One applied turn request: its `clientId` and the turn that served it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub client_id: String,
    pub turn_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChatMeta {
    /// Epoch ms per message index; shorter than the transcript for chats written before meta
    /// existed (those messages have no time).
    pub times: Vec<Option<i64>>,
    /// Indices of assistant messages a cancel or crash cut short.
    pub interrupted: Vec<usize>,
    pub applied: Vec<Applied>,
}

impl ChatMeta {
    /// The meta stored in a chat body (empty when there is none or it is unreadable).
    pub fn of(body: &Value) -> Self {
        body.get(KEY)
            .and_then(|m| serde_json::from_value(m.clone()).ok())
            .unwrap_or_default()
    }

    /// Stamps every message added since the last save with `now`.
    pub fn stamp(&mut self, s: &Session, now: i64) {
        let n = s.messages.len();
        if self.times.len() > n {
            self.times.truncate(n);
        }
        while self.times.len() < n {
            self.times.push(Some(now));
        }
    }

    /// `messages[..cut]` were replaced by one compaction summary.
    pub fn compacted(&mut self, cut: usize, now: i64) {
        let cut = cut.min(self.times.len());
        self.times.splice(..cut, [Some(now)]);
        self.interrupted = self
            .interrupted
            .iter()
            .filter(|&&i| i >= cut)
            .map(|&i| i - cut + 1)
            .collect();
    }

    pub fn time(&self, index: usize) -> Option<i64> {
        self.times.get(index).copied().flatten()
    }

    pub fn is_interrupted(&self, index: usize) -> bool {
        self.interrupted.contains(&index)
    }

    pub fn mark_interrupted(&mut self, index: usize) {
        if !self.interrupted.contains(&index) {
            self.interrupted.push(index);
        }
    }

    /// The turn that already served `client_id`, if any.
    pub fn applied(&self, client_id: &str) -> Option<&str> {
        self.applied
            .iter()
            .rev()
            .find(|a| a.client_id == client_id)
            .map(|a| a.turn_id.as_str())
    }

    pub fn remember(&mut self, client_id: &str, turn_id: &str) {
        self.applied.retain(|a| a.client_id != client_id);
        self.applied.push(Applied {
            client_id: client_id.to_owned(),
            turn_id: turn_id.to_owned(),
        });
    }

    /// The chat body for `s` with this meta attached.
    pub fn body(&self, s: &Session) -> serde_json::Result<Value> {
        let mut body = serde_json::to_value(s)?;
        if let Value::Object(map) = &mut body {
            map.insert(KEY.into(), serde_json::to_value(self)?);
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_agent::loops::{Kind, Msg};

    #[test]
    fn meta_round_trips_inside_the_session_body_and_tracks_compaction() {
        let mut s = Session::new("c1", Kind::Teach);
        s.push_user("a");
        s.push_user("b");
        let mut m = ChatMeta::default();
        m.stamp(&s, 10);
        s.messages.push(Msg::Assistant {
            text: "partial".into(),
            calls: vec![],
        });
        m.stamp(&s, 20);
        m.mark_interrupted(2);
        m.remember("c-1", "turn_1");
        let body = m.body(&s).unwrap();
        // The session still reads its own fields; the meta reads back unchanged.
        let back: Session = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(back, s);
        let read = ChatMeta::of(&body);
        assert_eq!(read, m);
        assert_eq!(
            (read.time(0), read.time(2), read.time(9)),
            (Some(10), Some(20), None)
        );
        assert!(read.is_interrupted(2) && !read.is_interrupted(1));
        assert_eq!(read.applied("c-1"), Some("turn_1"));

        let mut c = read.clone();
        c.compacted(2, 30);
        assert_eq!(c.times, [Some(30), Some(20)]);
        assert_eq!(c.interrupted, [1]);
        assert_eq!(ChatMeta::of(&Value::Null), ChatMeta::default());
    }

    #[test]
    fn client_ids_survive_long_sessions_and_latest_wins() {
        let mut m = ChatMeta::default();
        for i in 0..100 {
            m.remember(&format!("c{i}"), &format!("t{i}"));
        }
        assert_eq!(m.applied.len(), 100);
        assert_eq!(m.applied("c0"), Some("t0"));
        assert_eq!(m.applied("c99"), Some("t99"));
        m.remember("c99", "t100");
        assert_eq!(m.applied("c99"), Some("t100"));
        assert_eq!(m.applied.len(), 100);
    }
}
