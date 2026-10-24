//! The read of `HEAD` that every watch of one repository on this replica
//! shares, so many watches cost one read each poll, and a push through this
//! replica wakes them at once.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;
use std::time::Duration;

use tokio::sync::watch;

use super::super::Repo;

/// The poll of a repository's `HEAD`, while any watch holds it.
#[derive(Debug, Default)]
pub(crate) struct HeadSlot(Mutex<Weak<Poll>>);

/// A task that reads `HEAD` every poll and counts each move it sees. It
/// stops when the last watch that holds it drops.
#[derive(Debug)]
struct Poll {
    moves: Arc<watch::Sender<u64>>,
    task: tokio::task::AbortHandle,
}

impl Drop for Poll {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// One watch's view of the moves of `HEAD`.
pub(super) struct Moves {
    _poll: Arc<Poll>,
    changes: watch::Receiver<u64>,
}

impl Moves {
    /// Waits until `HEAD` may have moved since the last call.
    pub(super) async fn next(&mut self) {
        // The poll owns the sender, and this holds the poll, so the channel
        // never closes while a watch waits on it.
        let _ = self.changes.changed().await;
    }
}

impl HeadSlot {
    /// Wakes every watch of the repository, after a push through this
    /// replica.
    pub(crate) fn notify(&self) {
        let poll = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .upgrade();
        if let Some(poll) = poll {
            poll.moves.send_modify(|count| *count += 1);
        }
    }

    /// The moves of `HEAD`, from a poll every `every` that starts with the
    /// first watch.
    pub(super) fn subscribe(&self, repo: &Repo, every: Duration) -> Moves {
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(poll) = slot.upgrade() {
            let changes = poll.moves.subscribe();
            return Moves {
                _poll: poll,
                changes,
            };
        }
        let (moves, changes) = watch::channel(0);
        let moves = Arc::new(moves);
        let task = tokio::spawn(poll_head(repo.clone(), moves.clone(), every)).abort_handle();
        let poll = Arc::new(Poll { moves, task });
        *slot = Arc::downgrade(&poll);
        Moves {
            _poll: poll,
            changes,
        }
    }
}

/// Reads `HEAD` every `every` and counts a move whenever it differs from the
/// last read. A failed read counts too, so each watch reads the failure.
async fn poll_head(repo: Repo, moves: Arc<watch::Sender<u64>>, every: Duration) {
    let mut last = None;
    loop {
        tokio::time::sleep(every).await;
        let head = repo.head().await.ok();
        if head.is_none() || head != last {
            last = head;
            moves.send_modify(|count| *count += 1);
        }
    }
}
