//! Snapshot persistence: a versioned, checksummed binary format for saving
//! and loading a complete store.
//!
//! Format (version 2): 7-byte magic `CHRONO1`, one format-version byte,
//! a little-endian CRC32 of the body, then a bincode body containing the
//! configuration and all memories. The index is rebuilt on load.
//!
//! Writes are crash-safe: the snapshot is written to a temporary file in
//! the destination's directory and atomically renamed over the target, so
//! a crash mid-write can never destroy the previous snapshot.

use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::ChronoMind;
use crate::types::Memory;

const MAGIC: &[u8; 7] = b"CHRONO1";
const FORMAT_VERSION: u8 = 2;

#[derive(Serialize, Deserialize)]
struct SnapshotBody {
    config: Config,
    memories: Vec<Memory>,
}

/// Save a complete snapshot of `store` to `path`, atomically replacing any
/// existing file.
///
/// The write goes to a temporary file in `path`'s directory first and is
/// renamed into place only after it is fully written and flushed — a crash
/// at any point leaves either the old snapshot or the new one, never a
/// torn file.
#[instrument(skip(store))]
pub fn save_snapshot(store: &ChronoMind, path: &Path) -> Result<()> {
    let body = SnapshotBody {
        config: store.config().clone(),
        memories: store.snapshot(),
    };
    let encoded = bincode::serialize(&body)?;
    let checksum = crc32fast::hash(&encoded);

    let directory = path.parent().filter(|p| !p.as_os_str().is_empty());
    let mut temp = match directory {
        Some(dir) => tempfile::NamedTempFile::new_in(dir)?,
        None => tempfile::NamedTempFile::new_in(".")?,
    };
    temp.write_all(MAGIC)?;
    temp.write_all(&[FORMAT_VERSION])?;
    temp.write_all(&checksum.to_le_bytes())?;
    temp.write_all(&encoded)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| Error::Io(e.error))?;

    info!(memories = body.memories.len(), ?path, "snapshot saved");
    Ok(())
}

/// Load a store from a snapshot written by [`save_snapshot`].
///
/// The body checksum is verified before deserialization, so silent
/// corruption is rejected rather than half-loaded. The vector index is
/// rebuilt during load; load time scales with the number of stored
/// memories.
#[instrument]
pub fn load_snapshot(path: &Path) -> Result<ChronoMind> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut magic = [0u8; 7];
    reader
        .read_exact(&mut magic)
        .map_err(|_| Error::InvalidSnapshot("file too short to be a ChronoMind snapshot".into()))?;
    if &magic != MAGIC {
        return Err(Error::InvalidSnapshot(
            "bad magic bytes: not a ChronoMind snapshot".into(),
        ));
    }

    let mut version = [0u8; 1];
    reader
        .read_exact(&mut version)
        .map_err(|_| Error::InvalidSnapshot("missing format version".into()))?;
    if version[0] != FORMAT_VERSION {
        return Err(Error::InvalidSnapshot(format!(
            "unsupported format version {} (supported: {FORMAT_VERSION})",
            version[0]
        )));
    }

    let mut checksum_bytes = [0u8; 4];
    reader
        .read_exact(&mut checksum_bytes)
        .map_err(|_| Error::InvalidSnapshot("missing body checksum".into()))?;
    let expected_checksum = u32::from_le_bytes(checksum_bytes);

    let mut encoded = Vec::new();
    reader.read_to_end(&mut encoded)?;
    let actual_checksum = crc32fast::hash(&encoded);
    if actual_checksum != expected_checksum {
        return Err(Error::InvalidSnapshot(format!(
            "body checksum mismatch (expected {expected_checksum:08x}, \
             got {actual_checksum:08x}): the file is corrupt"
        )));
    }

    let body: SnapshotBody = bincode::deserialize(&encoded)?;
    let store = ChronoMind::new(body.config)?;
    let count = body.memories.len();
    for memory in body.memories {
        store.insert(memory)?;
    }

    info!(memories = count, ?path, "snapshot loaded");
    Ok(store)
}
