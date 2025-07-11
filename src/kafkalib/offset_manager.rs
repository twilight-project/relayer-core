use std::collections::BTreeSet;
use std::sync::Mutex;

/// Thread-safe tracker for Kafka offsets that have been *fully processed*
/// by your application but are **not yet** committed to the broker.
///
/// • Workers call `mark_done(offset)` when they finish business logic.  
/// • The commit loop calls `next_commit_offset()`;  
///   if it returns `Some(x)`, commit `<partition, x>` (which means x+1
///   will be the next offset delivered by Kafka).
///
/// Internally we keep a BTreeSet so look-ups & removals stay `O(log n)`,
/// and we advance `committed` only when there are *no gaps*.
#[derive(Debug)]
pub struct OffsetManager {
    inner: Mutex<State>,
}

#[derive(Debug)]
struct State {
    committed: i64,         // highest *already committed* offset
    pending: BTreeSet<i64>, // finished but not yet committed
}

impl OffsetManager {
    /// `start` is the last offset you *already* committed when the program
    /// started (or -1 if you start from the very beginning).
    pub fn new(start: i64) -> Self {
        Self {
            inner: Mutex::new(State {
                committed: start,
                pending: BTreeSet::new(),
            }),
        }
    }

    /// Call from your worker threads when they have **successfully**
    /// processed the message at `offset`.
    pub fn mark_done(&self, offset: i64) {
        let mut state = self.inner.lock().unwrap();
        state.pending.insert(offset);
    }

    /// Returns `Some(highest_contiguous_offset)` if we can move the commit
    /// pointer forward, otherwise `None`.  
    /// Call this from a *single* commit thread / task.
    pub fn next_commit_offset(&self) -> Option<i64> {
        let mut state = self.inner.lock().unwrap();
        let mut advanced = false;

        loop {
            let target = state.committed + 1;

            // `remove` returns `true` if `target` existed and was removed.
            if state.pending.remove(&target) {
                state.committed = target;
                advanced = true;
            } else {
                // gap found → cannot advance further
                break;
            }
        }

        if advanced {
            Some(state.committed) // commit this offset
        } else {
            None // nothing contiguous yet
        }
    }
}
