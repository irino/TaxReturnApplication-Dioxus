//! JSON load/save for one-file-per-fiscal-year ledgers.

use crate::core::journal::LedgerDocument;
use crate::storage::paths::{ledgers_dir, PathError};
use crate::storage::sqlite_store::SqliteError;
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
    #[error("SQLiteエラー: {0}")]
    Sqlite(#[from] SqliteError),
}

fn year_path(dir: &Path, year: i32) -> PathBuf {
    dir.join(format!("{year}.json"))
}

/// Load a fiscal year from SQLite (with automatic migration from legacy JSON if needed).
/// Missing records → empty document for that year.
pub fn load_year(year: i32) -> Result<LedgerDocument, StorageError> {
    let sqlite_file = crate::storage::sqlite_store::sqlite_db_path()?;
    if sqlite_file.exists() {
        let conn = crate::storage::sqlite_store::open_sqlite_connection(&sqlite_file)?;
        let doc = crate::storage::sqlite_store::load_year_from_sqlite(&conn, year)?;
        if !doc.entries.is_empty() || doc.taxpayer != crate::core::journal::TaxpayerInfo::default() {
            return Ok(doc);
        }
    }

    // SQLiteにデータがない場合、レガシーJSONを確認
    let dir = ledgers_dir()?;
    let path = year_path(&dir, year);
    if path.exists() {
        // レガシーJSONが存在する場合はロードし、SQLiteへ移行保存
        let doc = load_year_from_dir(&dir, year)?;
        if let Ok(mut conn) = crate::storage::sqlite_store::open_sqlite_connection(&sqlite_file) {
            let _ = crate::storage::sqlite_store::save_year_to_sqlite(&mut conn, &doc);
        }
        return Ok(doc);
    }

    Ok(LedgerDocument::new(year))
}

/// Save a fiscal year into SQLite (primary) and JSON (backup).
pub fn save_year(doc: &LedgerDocument) -> Result<(), StorageError> {
    // 1. SQLite単一ファイルへ保存
    let sqlite_file = crate::storage::sqlite_store::sqlite_db_path()?;
    let mut conn = crate::storage::sqlite_store::open_sqlite_connection(&sqlite_file)?;
    crate::storage::sqlite_store::save_year_to_sqlite(&mut conn, doc)?;

    // 2. レガシーJSONファイルへも保存（バックアップ互換性）
    if let Ok(dir) = ledgers_dir() {
        let _ = save_year_to_dir(&dir, doc);
    }

    Ok(())
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
