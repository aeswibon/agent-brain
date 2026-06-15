//! Restore a fact superseded during sync import.

use anyhow::{bail, Context, Result};

use crate::db::store::{content_hash, BrainStore};
use crate::embed::Embedder;

pub fn restore_conflict(
    store: &BrainStore,
    embedder: &Embedder,
    conflict_id: &str,
) -> Result<String> {
    let row = store
        .get_conflict(conflict_id)?
        .with_context(|| format!("conflict not found: {conflict_id}"))?;

    if row.restored {
        bail!("conflict already restored: {conflict_id}");
    }

    // Free unique(content_hash, scope) slot held by the superseded loser row.
    store.delete_fact_by_id(&row.loser_id)?;

    let embedding = embedder.embed_one(&format!("{} {}", row.topic, row.loser_fact))?;
    let hash = content_hash(&row.loser_fact);
    let res = store.store_fact_full(
        &row.topic,
        &row.loser_fact,
        &row.scope,
        row.scope_key.as_deref(),
        0.95,
        "user",
        &hash,
        &embedding,
        "positive",
        None,
    )?;

    store.mark_conflict_restored(conflict_id)?;
    Ok(res.id)
}
