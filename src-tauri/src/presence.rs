use std::collections::HashMap;
use std::sync::Mutex;

/// Ref-counted online auth users (multiple tabs allowed).
#[derive(Default)]
pub struct PresenceRegistry {
    inner: Mutex<HashMap<String, usize>>,
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

    pub fn online_user_ids(&self) -> Vec<String> {
        let map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut ids: Vec<_> = map.keys().cloned().collect();
        ids.sort();
        ids
    }

    pub fn is_online(&self, user_id: &str) -> bool {
        let map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        map.get(user_id).copied().unwrap_or(0) > 0
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
}
