use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Ref-counted online auth users (multiple tabs allowed), plus HTTP TTL heartbeats
/// for clients that cannot hold a WebSocket (e.g. WorkPet via Connecter).
#[derive(Default)]
pub struct PresenceRegistry {
    inner: Mutex<HashMap<String, usize>>,
    http_until: Mutex<HashMap<String, i64>>,
}

impl PresenceRegistry {
    pub fn connect(&self, user_id: &str) -> bool {
        if user_id.is_empty() {
            return false;
        }
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let entry = map.entry(user_id.to_string()).or_insert(0);
        *entry += 1;
        *entry == 1
    }

    pub fn disconnect(&self, user_id: &str) -> bool {
        if user_id.is_empty() {
            return false;
        }
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(count) = map.get_mut(user_id) else {
            return false;
        };
        if *count <= 1 {
            map.remove(user_id);
            true
        } else {
            *count -= 1;
            false
        }
    }

    /// Mark `user_id` online until `now + ttl_ms`. Returns true if they were offline.
    pub fn touch_http(&self, user_id: &str, ttl_ms: i64) -> bool {
        if user_id.is_empty() || ttl_ms <= 0 {
            return false;
        }
        let was = self.is_online(user_id);
        let until = now_ms().saturating_add(ttl_ms);
        let mut http = self.http_until.lock().unwrap_or_else(|e| e.into_inner());
        http.insert(user_id.to_string(), until);
        !was
    }

    pub fn online_user_ids(&self) -> Vec<String> {
        let now = now_ms();
        let mut set = HashSet::new();
        {
            let map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            for (k, c) in map.iter() {
                if *c > 0 {
                    set.insert(k.clone());
                }
            }
        }
        {
            let mut http = self.http_until.lock().unwrap_or_else(|e| e.into_inner());
            http.retain(|_, until| *until > now);
            for k in http.keys() {
                set.insert(k.clone());
            }
        }
        let mut ids: Vec<_> = set.into_iter().collect();
        ids.sort();
        ids
    }

    pub fn is_online(&self, user_id: &str) -> bool {
        {
            let map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if map.get(user_id).copied().unwrap_or(0) > 0 {
                return true;
            }
        }
        let now = now_ms();
        let http = self.http_until.lock().unwrap_or_else(|e| e.into_inner());
        http.get(user_id).copied().unwrap_or(0) > now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refcount_connect_disconnect() {
        let p = PresenceRegistry::default();
        assert!(p.connect("u1"));
        assert!(!p.connect("u1"));
        assert!(p.is_online("u1"));
        assert!(!p.disconnect("u1"));
        assert!(p.is_online("u1"));
        assert!(p.disconnect("u1"));
        assert!(!p.is_online("u1"));
    }

    #[test]
    fn http_ttl_marks_online_then_expires() {
        let p = PresenceRegistry::default();
        assert!(p.touch_http("u2", 80));
        assert!(!p.touch_http("u2", 80));
        assert!(p.is_online("u2"));
        assert_eq!(p.online_user_ids(), vec!["u2".to_string()]);
        std::thread::sleep(std::time::Duration::from_millis(120));
        assert!(!p.is_online("u2"));
        assert!(p.online_user_ids().is_empty());
    }

    #[test]
    fn http_and_websocket_merge() {
        let p = PresenceRegistry::default();
        p.connect("ws");
        p.touch_http("http", 60_000);
        let ids = p.online_user_ids();
        assert_eq!(ids, vec!["http".to_string(), "ws".to_string()]);
    }
}
