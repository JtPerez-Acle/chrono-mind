//! Snapshot persistence: a versioned binary format for saving and loading
//! a complete store.
//!
//! Format: 7-byte magic `CHRONO1`, one format-version byte, then a bincode
//! body containing the configuration and all memories. The index is rebuilt
//! on load.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::ChronoMind;
use crate::types::Memory;

const MAGIC: &[u8; 7] = b"CHRONO1";
const FORMAT_VERSION: u8 = 1;

#[derive(Serialize, Deserialize)]
struct SnapshotBody {
    config: Config,
    memories: Vec<Memory>,
}

/// Save a complete snapshot of `store` to `path`, overwriting any existing
/// file.
#[instrument(skip(store))]
pub fn save_snapshot(store: &ChronoMind, path: &Path) -> Result<()> {
    let body = SnapshotBody {
        config: store.config().clone(),
        memories: store.snapshot(),
    };

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(MAGIC)?;
    writer.write_all(&[FORMAT_VERSION])?;
    bincode::serialize_into(&mut writer, &body)?;
    writer.flush()?;

    info!(memories = body.memories.len(), ?path, "snapshot saved");
    Ok(())
}

/// Load a store from a snapshot written by [`save_snapshot`].
///
/// The vector index is rebuilt during load, so load time scales with the
/// number of stored memories.
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

    let body: SnapshotBody = bincode::deserialize_from(&mut reader)?;
    let store = ChronoMind::new(body.config)?;
    let count = body.memories.len();
    for memory in body.memories {
        store.insert(memory)?;
    }

    info!(memories = count, ?path, "snapshot loaded");
    Ok(store)
}
