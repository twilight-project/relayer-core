#![allow(dead_code)]
#![allow(unused_imports)]

mod codec;
mod current;
mod legacy;
mod orchestration;

// Re-export everything that was previously public
pub use self::current::{OrderDBSnapShot, OrderDBSnapShotLO, OrderDBSnapShotTO, SnapshotDB};
pub use self::legacy::{
    SnapshotDBOld, SnapshotDBOldV1, SnapshotDBOldV10, SnapshotDBOldV2, SnapshotDBOldV3,
    SnapshotDBOldV5, SnapshotDBOldV6, SnapshotDBOldV7, SnapshotDBOldV9,
};
pub use self::orchestration::{create_snapshot_data, load_from_snapshot, snapshot};

// Shared macro used across submodules
macro_rules! lock_or_err {
    ($global:expr, $name:expr) => {
        $global.lock().map_err(|e| {
            crate::log_heartbeat!(error, "Error locking {} : {:?}", $name, e);
            e.to_string()
        })?
    };
}
pub(crate) use lock_or_err;
