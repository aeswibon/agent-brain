//! Combined sync operator status (git + conflict log).

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::db::store::BrainStore;
use crate::settings::GitSyncSettings;

use super::git::{git_status, GitSyncStatus};

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub git: GitSyncStatus,
    pub unresolved_conflicts: usize,
    pub recent_conflicts: Vec<serde_json::Value>,
}

pub fn sync_status(home: &Path, git: &GitSyncSettings, store: &BrainStore) -> Result<SyncStatus> {
    Ok(SyncStatus {
        git: git_status(home, git)?,
        unresolved_conflicts: store.count_unresolved_conflicts()?,
        recent_conflicts: store.list_conflicts(5)?,
    })
}
