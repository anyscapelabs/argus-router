use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);
const MAX_TRACKED: usize = 10_000;

#[derive(Default)]
pub struct RateWindow {
    hits: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl RateWindow {
    pub fn hit(&self, user_id: &str, quota: i64) -> (i64, i64) {
        if quota <= 0 {
            return (0, 0);
        }
        let now = Instant::now();
        let mut hits = self.hits.lock().expect("rate window poisoned");
        if hits.len() > MAX_TRACKED {
            hits.retain(|_, window| window.back().is_some_and(|last| now - *last < WINDOW));
        }
        let window = hits.entry(user_id.to_owned()).or_default();
        while window.front().is_some_and(|at| now - *at >= WINDOW) {
            window.pop_front();
        }
        window.push_back(now);
        (window.len() as i64, quota)
    }
}
