use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Session {
    pub id: u64,
    pub addr: Option<String>,
    pub token: String,
}

// #[async_trait]
pub trait SessionStore: Send + Sync {
    fn create_session(&self, username: &str, token: String);

    fn get_session(&self, username: &str, id: u64) -> Option<Session>;

    fn list_sessions(&self, username: &str) -> Vec<Session>;

    fn delete_session(&self, username: &str, id: u64);
}

#[derive(Default)]
pub struct InMemorySessionStore {
    sessions: Arc<Mutex<HashMap<String, HashMap<u64, Session>>>>,
    next_session_id: AtomicU64,
}

// #[async_trait]
impl SessionStore for InMemorySessionStore {
    fn create_session(&self, username: &str, token: String) {
        let id = self
            .next_session_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.sessions
            .lock()
            .unwrap()
            .entry(username.to_string())
            .or_default()
            .insert(
                id,
                Session {
                    id,
                    addr: None,
                    token,
                },
            );
    }

    fn get_session(&self, username: &str, id: u64) -> Option<Session> {
        self.sessions
            .lock()
            .unwrap()
            .get(username)
            .and_then(|sessions| sessions.get(&id))
            .cloned()
    }

    fn list_sessions(&self, username: &str) -> Vec<Session> {
        self.sessions
            .lock()
            .unwrap()
            .get(username)
            .map(|sessions| sessions.values().cloned().collect())
            .unwrap_or_default()
    }

    fn delete_session(&self, username: &str, id: u64) {
        if let Some(sessions) = self.sessions.lock().unwrap().get_mut(username) {
            sessions.remove(&id);
        }
    }
}
