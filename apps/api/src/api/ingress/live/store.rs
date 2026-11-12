use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

const LAST_REPLY_TTL_SECS: u64 = 120;

struct LastReplyEntry {
    user_id: String,
    text: String,
    stored_at: SystemTime,
}

#[derive(Default)]
pub struct LiveSessionStore {
    inner: Mutex<HashMap<String, LastReplyEntry>>,
}

impl LiveSessionStore {
    pub fn store(&self, session_id: &str, user_id: &str, text: String) {
        let mut map = self.inner.lock().expect("LiveSessionStore lock poisoned");
        map.retain(|_, v| {
            v.stored_at
                .elapsed()
                .map(|e| e.as_secs() < LAST_REPLY_TTL_SECS)
                .unwrap_or(false)
        });
        map.insert(
            session_id.to_owned(),
            LastReplyEntry {
                user_id: user_id.to_owned(),
                text,
                stored_at: SystemTime::now(),
            },
        );
    }

    pub fn take(&self, session_id: &str, user_id: &str) -> Option<String> {
        let mut map = self.inner.lock().expect("LiveSessionStore lock poisoned");
        let entry = map.remove(session_id)?;
        if entry.user_id != user_id {
            return None;
        }
        if entry
            .stored_at
            .elapsed()
            .map(|e| e.as_secs())
            .unwrap_or(u64::MAX)
            >= LAST_REPLY_TTL_SECS
        {
            return None;
        }
        Some(entry.text)
    }
}
