//! Short-lived in-memory store for voice→text fallback replies, keyed by
//! session id. Entries expire after [`LAST_REPLY_TTL_SECS`].

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

/// TTL for entries in `LiveSessionStore` (2 minutes).
const LAST_REPLY_TTL_SECS: u64 = 120;

struct LastReplyEntry {
    text: String,
    stored_at: SystemTime,
}

/// Stores the last model text per session id so the desktop app can recover
/// a partial reply when a voice session ends early.
#[derive(Default)]
pub struct LiveSessionStore {
    inner: Mutex<HashMap<String, LastReplyEntry>>,
}

impl LiveSessionStore {
    /// Stores the last model text for `session_id`, evicting expired entries.
    pub fn store(&self, session_id: &str, text: String) {
        let mut map = self.inner.lock().expect("LiveSessionStore lock poisoned");
        // Opportunistic TTL eviction.
        map.retain(|_, v| {
            v.stored_at
                .elapsed()
                .map(|e| e.as_secs() < LAST_REPLY_TTL_SECS)
                .unwrap_or(false)
        });
        map.insert(
            session_id.to_owned(),
            LastReplyEntry {
                text,
                stored_at: SystemTime::now(),
            },
        );
    }

    /// Takes (removes and returns) the stored reply, returning `None` if absent
    /// or expired.
    pub fn take(&self, session_id: &str) -> Option<String> {
        let mut map = self.inner.lock().expect("LiveSessionStore lock poisoned");
        let entry = map.remove(session_id)?;
        if entry
            .stored_at
            .elapsed()
            .map(|e| e.as_secs())
            .unwrap_or(u64::MAX)
            >= LAST_REPLY_TTL_SECS
        {
            return None; // expired
        }
        Some(entry.text)
    }
}
