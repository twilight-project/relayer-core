use crc32fast::Hasher as Crc32Hasher;
use std::fs::{self, File, OpenOptions};
use std::io::Cursor;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::config::*;
use crate::db::*;

use super::current::{SnapshotDB, SNAPSHOT_FORMAT_VERSION};
use super::legacy::*;

/// Decompress zstd payload, returning the raw bincode bytes.
fn zstd_decompress(compressed: &[u8]) -> Result<Vec<u8>, String> {
    zstd::decode_all(Cursor::new(compressed))
        .map_err(|e| format!("zstd decompression failed: {:?}", e))
}

/// Try to decode a snapshot file that has the new version+checksum header.
/// Returns `None` if the data is too short or the version marker doesn't match
/// any known version (which means it's probably a legacy file).
pub(crate) fn decode_versioned_snapshot(data: &[u8]) -> Result<Option<SnapshotDB>, String> {
    if data.len() < 8 {
        return Ok(None); // too short to have header — treat as legacy
    }

    let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let stored_crc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let payload = &data[8..];

    // Verify CRC32 checksum (covers the raw bytes on disk, whether compressed or not)
    let mut hasher = Crc32Hasher::new();
    hasher.update(payload);
    let computed_crc = hasher.finalize();
    if stored_crc != computed_crc {
        if version >= 8 {
            if *ALLOW_LEGACY_SNAPSHOT_FALLBACK {
                crate::log_heartbeat!(
                    warn,
                    "CRC mismatch for apparent version {} (stored={:#010x}, computed={:#010x}). \
                     ALLOW_LEGACY_SNAPSHOT_FALLBACK=true, falling back to legacy decoding.",
                    version,
                    stored_crc,
                    computed_crc
                );
                return Ok(None);
            }
            return Err(format!(
                "Snapshot checksum mismatch: stored={:#010x}, computed={:#010x}. File may be corrupt. \
                 Set ALLOW_LEGACY_SNAPSHOT_FALLBACK=true to attempt legacy decoding.",
                stored_crc, computed_crc
            ));
        }
        crate::log_heartbeat!(
            info,
            "CRC mismatch for apparent version {} — likely a legacy (pre-header) snapshot, falling back",
            version
        );
        return Ok(None);
    }

    match version {
        11 => {
            let decompressed = zstd_decompress(payload)?;
            bincode::deserialize::<SnapshotDB>(&decompressed)
                .map(|s| Some(s))
                .map_err(|e| format!("V11 deserialize failed: {:#?}", e))
        }
        10 => {
            let decompressed = zstd_decompress(payload)?;
            bincode::deserialize::<SnapshotDBOldV10>(&decompressed)
                .map(|s| Some(s.migrate_to_new()))
                .map_err(|e| format!("V10 deserialize failed: {:#?}", e))
        }
        9 => {
            let decompressed = zstd_decompress(payload)?;
            bincode::deserialize::<SnapshotDBOldV9>(&decompressed)
                .map(|s| Some(s.migrate_to_new().migrate_to_new()))
                .map_err(|e| format!("V9 deserialize failed: {:#?}", e))
        }
        8 => {
            let decompressed = zstd_decompress(payload)?;
            bincode::deserialize::<SnapshotDBOldV7>(&decompressed)
                .map(|s| Some(s.migrate_to_new().migrate_to_new().migrate_to_new()))
                .map_err(|e| format!("V8 deserialize failed: {:#?}", e))
        }
        7 => bincode::deserialize::<SnapshotDBOldV7>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V7 deserialize failed: {:#?}", e)),
        6 => bincode::deserialize::<SnapshotDBOldV6>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V6 deserialize failed: {:#?}", e)),
        5 => bincode::deserialize::<SnapshotDBOldV5>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V5 deserialize failed: {:#?}", e)),
        4 => bincode::deserialize::<SnapshotDBOld>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V4 deserialize failed: {:#?}", e)),
        3 => bincode::deserialize::<SnapshotDBOldV3>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V3 deserialize failed: {:#?}", e)),
        2 => bincode::deserialize::<SnapshotDBOldV2>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V2 deserialize failed: {:#?}", e)),
        1 => bincode::deserialize::<SnapshotDBOldV1>(payload)
            .map(|s| {
                Some(
                    s.migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new()
                        .migrate_to_new(),
                )
            })
            .map_err(|e| format!("V1 deserialize failed: {:#?}", e)),
        _ => Ok(None), // unknown version — treat as legacy
    }
}

/// Fall back to the old trial-and-error deserialization for legacy snapshot
/// files that were written without a version header.
pub(crate) fn decode_legacy_snapshot(data: &[u8]) -> Result<SnapshotDB, String> {
    if let Ok(snap) = bincode::deserialize::<SnapshotDB>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V11 (SL/TP queues)");
        return Ok(snap);
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV10>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V10 (split SL/TP), migrating");
        return Ok(snap.migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV9>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V9 (merged SLTP), migrating");
        return Ok(snap.migrate_to_new().migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV7>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V7, migrating");
        return Ok(snap.migrate_to_new().migrate_to_new().migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV6>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V6, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV5>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V5, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOld>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V4, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV3>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V3, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV2>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V2, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    if let Ok(snap) = bincode::deserialize::<SnapshotDBOldV1>(data) {
        crate::log_heartbeat!(info, "Decoded legacy snapshot as V1, migrating");
        return Ok(snap
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new()
            .migrate_to_new());
    }
    Err("All legacy snapshot decode attempts failed (V8-V1)".to_string())
}

/// Atomically replace the snapshot file and notify the offset channel.
pub(crate) fn write_snapshot_atomically(
    encoded: &[u8],
    final_path: &PathBuf,
    tx_consumed: &crossbeam_channel::Sender<(i32, i64)>,
    offset: (i32, i64),
) -> io::Result<()> {
    // 1. Write to temporary file
    let tmp_path = final_path.with_extension("new");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp_path)?;
    file.write_all(encoded)?;
    file.sync_all()?;

    // 2. Atomically move temp → final (fs::rename replaces atomically on Unix)
    fs::rename(&tmp_path, final_path)?;

    // 3. fsync the directory entry so the rename itself is durable
    if let Some(parent) = final_path.parent() {
        let dir = File::open(parent)?;
        dir.sync_all()?;
    }

    // 4. Push committed offset downstream
    if let Err(e) = tx_consumed.send(offset) {
        crate::log_heartbeat!(error, "Error pushing committed offset downstream: {:?}", e);
    }

    Ok(())
}
