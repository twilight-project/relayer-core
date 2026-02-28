use crate::config::PERSISTENT_QUEUE_PATH;
use serde_derive::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

lazy_static! {
    pub static ref EVENT_QUEUE: PersistentEventQueue = PersistentEventQueue::new();
}

#[derive(Serialize, Deserialize)]
pub struct QueueEntry {
    pub topic: String,
    pub key: String,
    pub data: Vec<u8>,
}

pub struct PersistentEventQueue {
    db: RwLock<sled::Db>,
    counter: AtomicU64,
    remove_counter: AtomicU64,
}

unsafe impl Send for PersistentEventQueue {}
unsafe impl Sync for PersistentEventQueue {}

impl PersistentEventQueue {
    pub fn new() -> Self {
        let db = sled::open(PERSISTENT_QUEUE_PATH.as_str())
            .expect("Failed to open persistent event queue database");

        let counter = match db.last() {
            Ok(Some((last_key, _))) => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&last_key);
                AtomicU64::new(u64::from_be_bytes(buf) + 1)
            }
            _ => AtomicU64::new(0),
        };

        let pending = db.len();
        if pending > 0 {
            crate::log_heartbeat!(
                warn,
                "PERSISTENT_QUEUE: Opened with {} pending events from previous run",
                pending
            );
        }

        PersistentEventQueue {
            db: RwLock::new(db),
            counter,
            remove_counter: AtomicU64::new(0),
        }
    }

    pub fn push(&self, topic: String, key: String, data: Vec<u8>) -> Result<u64, String> {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        let id_bytes = id.to_be_bytes();

        let entry = QueueEntry { topic, key, data };
        let value = serde_json::to_vec(&entry).map_err(|e| e.to_string())?;

        let db = self.db.read().map_err(|e| e.to_string())?;
        db.insert(id_bytes, value).map_err(|e| e.to_string())?;
        db.flush().map_err(|e| e.to_string())?;

        Ok(id)
    }

    pub fn remove(&self, id: u64) -> Result<(), String> {
        let id_bytes = id.to_be_bytes();
        let db = self.db.read().map_err(|e| e.to_string())?;
        db.remove(id_bytes).map_err(|e| e.to_string())?;
        drop(db);

        let count = self.remove_counter.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= 10_000 {
            self.reset_if_empty();
        }
        Ok(())
    }

    pub fn drain_pending(&self) -> Vec<(u64, QueueEntry)> {
        let mut entries = Vec::new();
        let db = self
            .db
            .read()
            .expect("Failed to acquire read lock on event queue");
        for item in db.iter() {
            match item {
                Ok((key, value)) => {
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(&key);
                    let id = u64::from_be_bytes(buf);

                    match serde_json::from_slice::<QueueEntry>(&value) {
                        Ok(entry) => entries.push((id, entry)),
                        Err(e) => {
                            crate::log_heartbeat!(
                                error,
                                "PERSISTENT_QUEUE: Failed to deserialize entry {}: {}",
                                id,
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    crate::log_heartbeat!(error, "PERSISTENT_QUEUE: Failed to read entry: {}", e);
                }
            }
        }
        entries
    }

    pub fn pending_count(&self) -> usize {
        let db = self
            .db
            .read()
            .expect("Failed to acquire read lock on event queue");
        db.len()
    }

    /// Reclaim disk space by dropping and recreating the sled DB when the queue is empty.
    /// sled's log-structured storage never compacts on its own, so the DB file grows
    /// unboundedly even though entries are constantly inserted and removed.
    pub fn reset_if_empty(&self) {
        let mut db = match self.db.write() {
            Ok(guard) => guard,
            Err(e) => {
                crate::log_heartbeat!(
                    error,
                    "PERSISTENT_QUEUE: Failed to acquire write lock for reset: {}",
                    e
                );
                return;
            }
        };

        if !db.is_empty() {
            return;
        }

        let path = PERSISTENT_QUEUE_PATH.as_str();

        // Drop the current DB to release file handles
        drop(std::mem::replace(
            &mut *db,
            sled::Config::new()
                .temporary(true)
                .open()
                .expect("Failed to open temporary sled DB"),
        ));

        // Verify the directory only contains known sled files before deleting
        if !Self::is_sled_directory_only(path) {
            crate::log_heartbeat!(
                error,
                "PERSISTENT_QUEUE: Aborting reset — directory '{}' contains non-sled files",
                path
            );
            // Reopen the original DB since we already dropped it
            *db = sled::open(path).expect("Failed to reopen persistent event queue database");
            return;
        }

        // Delete the DB directory
        if let Err(e) = std::fs::remove_dir_all(path) {
            crate::log_heartbeat!(
                warn,
                "PERSISTENT_QUEUE: Failed to remove DB directory during reset: {}",
                e
            );
        }

        // Reopen a fresh DB at the same path
        *db =
            sled::open(path).expect("Failed to reopen persistent event queue database after reset");

        // Reset counter since the DB is empty
        self.counter.store(0, Ordering::SeqCst);
        self.remove_counter.store(0, Ordering::Relaxed);

        crate::log_heartbeat!(info, "PERSISTENT_QUEUE: DB reset — disk space reclaimed");
    }

    /// Returns true if every entry in the directory is a known sled file/subdirectory.
    fn is_sled_directory_only(path: &str) -> bool {
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => return false,
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return false,
            };

            let name = entry.file_name();
            let name = name.to_string_lossy();

            // Known sled files: db, conf, blobs (dir), snap.* (snapshot files)
            let is_sled =
                name == "db" || name == "conf" || name == "blobs" || name.starts_with("snap.");

            if !is_sled {
                crate::log_heartbeat!(
                    warn,
                    "PERSISTENT_QUEUE: Unexpected file in DB directory: '{}'",
                    name
                );
                return false;
            }
        }

        true
    }
}
