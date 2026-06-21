//! AES-256-GCM encryption at rest for the brain SQLite database (SQLCipher pragma).

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::secrets::{get_secret, set_secret};

const DB_KEY_NAME: &str = "AGENT_BRAIN_DB_MASTER_KEY";

pub fn encryption_enabled() -> bool {
    std::env::var("AGENT_BRAIN_ENCRYPT_DB")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub fn get_or_create_db_key() -> Result<String> {
    if let Some(key) = get_secret(DB_KEY_NAME)? {
        return Ok(key);
    }
    let key = uuid::Uuid::now_v7().simple().to_string();
    set_secret(DB_KEY_NAME, &key)?;
    Ok(key)
}

pub fn apply_encryption_key(conn: &Connection) -> Result<()> {
    if !encryption_enabled() {
        return Ok(());
    }
    let key = get_or_create_db_key()?;
    conn.execute("PRAGMA key = ?1", rusqlite::params![key])
        .context("apply SQLCipher key")?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .context("verify encrypted database readability")?;
    Ok(())
}

pub fn enable_encryption_on_existing(conn: &Connection) -> Result<()> {
    if !encryption_enabled() {
        return Ok(());
    }
    let key = get_or_create_db_key()?;
    conn.execute("PRAGMA rekey = ?1", rusqlite::params![key])
        .context("rekey database for encryption at rest")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encryption_flag_from_env() {
        std::env::set_var("AGENT_BRAIN_ENCRYPT_DB", "1");
        assert!(encryption_enabled());
        std::env::remove_var("AGENT_BRAIN_ENCRYPT_DB");
    }
}
