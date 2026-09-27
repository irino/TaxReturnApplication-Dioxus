//! JSON load/save for one-file-per-fiscal-year ledgers.

use crate::core::journal::LedgerDocument;
use crate::storage::paths::{ledgers_dir, PathError};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("入出力エラー: {0}")]
    Io(#[from] io::Error),
    #[error("JSONエラー: {0}")]
    Json(#[from] serde_json::Error),
}

fn year_path(dir: &Path, year: i32) -> PathBuf {
    dir.join(format!("{year}.json"))
}

/// Load a fiscal year from the default app data directory.
/// Missing file → empty document for that year.
pub fn load_year(year: i32) -> Result<LedgerDocument, StorageError> {
    load_year_from_dir(&ledgers_dir()?, year)
}

/// Save a fiscal year into the default app data directory (atomic write).
pub fn save_year(doc: &LedgerDocument) -> Result<(), StorageError> {
    save_year_to_dir(&ledgers_dir()?, doc)
}

/// Load from an explicit ledgers directory (testable).
pub fn load_year_from_dir(dir: &Path, year: i32) -> Result<LedgerDocument, StorageError> {
    let path = year_path(dir, year);
    if !path.exists() {
        return Ok(LedgerDocument::new(year));
    }
    let raw = fs::read_to_string(&path)?;
    let doc: LedgerDocument = serde_json::from_str(&raw)?;
    Ok(doc)
}

/// Atomic save: write `*.json.tmp` then rename over the target.
pub fn save_year_to_dir(dir: &Path, doc: &LedgerDocument) -> Result<(), StorageError> {
    fs::create_dir_all(dir)?;
    let path = year_path(dir, doc.fiscal_year);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(doc)?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::journal::{AccountCode, JournalEntry};
    use chrono::NaiveDate;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("tax_return_test_{nanos}"))
    }

    #[test]
    fn load_missing_returns_empty_document() {
        let dir = temp_dir();
        let doc = load_year_from_dir(&dir, 2025).unwrap();
        assert_eq!(doc.fiscal_year, 2025);
        assert!(doc.entries.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = temp_dir();
        let mut doc = LedgerDocument::new(2025);
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            12_345,
            "保存テスト",
        ));
        save_year_to_dir(&dir, &doc).unwrap();
        let loaded = load_year_from_dir(&dir, 2025).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].amount, 12_345);
        assert_eq!(loaded.entries[0].description, "保存テスト");
        let _ = fs::remove_dir_all(&dir);
    }
}
