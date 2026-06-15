//! S3 encrypted cloud sync (groundwork — full implementation in a later release).

use anyhow::{bail, Result};

use crate::settings::CloudSyncSettings;

pub fn cloud_push(_settings: &CloudSyncSettings) -> Result<()> {
    bail!(
        "sync cloud push is not implemented yet; configure [sync.cloud] for a future release"
    )
}

pub fn cloud_pull(_settings: &CloudSyncSettings) -> Result<()> {
    bail!(
        "sync cloud pull is not implemented yet; configure [sync.cloud] for a future release"
    )
}
