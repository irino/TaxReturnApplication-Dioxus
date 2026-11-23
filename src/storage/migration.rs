//! Automated Migration Engine from Legacy Per-Year JSON to Unified SQLite.
//!
//! Scans for legacy `{year}.json` files, deserializes all historical documents,
//! and atomically imports them into the single GnuCash-compatible SQLite database.

use crate::storage::json_store::load_year_from_dir;
use crate::storage::paths::ledgers_dir;
use crate::storage::sqlite_store::{open_sqlite_connection, save_year_to_sqlite, sqlite_db_path, SqliteError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MigrationReport {
    pub migrated_years: Vec<i32>,
    pub total_entries: usize,
    pub messages: Vec<String>,
}

/// 指定ディレクトリ内の `{year}.json` ファイルから存在する年度一覧を検出
pub fn detect_legacy_json_years(dir: &Path) -> Vec<i32> {
    let mut years = Vec::new();
    if !dir.is_dir() {
        return years;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(y) = stem.parse::<i32>() {
                        if (1900..=2100).contains(&y)
                            && path.extension().and_then(|e| e.to_str()) == Some("json")
                        {
                            years.push(y);
                        }
                    }
                }
            }
        }
    }

    years.sort_unstable();
    years
}

/// 既定のレガシーJSONディレクトリから単一SQLiteへの一括自動マイグレーションを実行
pub fn auto_migrate_if_needed() -> Result<Option<MigrationReport>, SqliteError> {
    let json_dir = match ledgers_dir() {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    let years = detect_legacy_json_years(&json_dir);
    if years.is_empty() {
        return Ok(None);
    }

    let db_path = sqlite_db_path()?;
    let report = migrate_json_directory_to_sqlite(&json_dir, &db_path)?;
    Ok(Some(report))
}

/// JSONディレクトリ内の全年度データを指定のSQLiteデータベースへ一括移行
pub fn migrate_json_directory_to_sqlite(
    json_dir: &Path,
    sqlite_path: &Path,
) -> Result<MigrationReport, SqliteError> {
    let years = detect_legacy_json_years(json_dir);
    let mut conn = open_sqlite_connection(sqlite_path)?;

    let mut report = MigrationReport::default();

    for y in years {
        match load_year_from_dir(json_dir, y) {
            Ok(doc) => {
                let entry_count = doc.entries.len();
                save_year_to_sqlite(&mut conn, &doc)?;
                report.migrated_years.push(y);
                report.total_entries += entry_count;
                report.messages.push(format!(
                    "{y}年度のデータ（仕訳{entry_count}件）をSQLiteへ移行しました"
                ));
            }
            Err(e) => {
                report
                    .messages
                    .push(format!("{y}年度の読み込みに失敗しました: {e}"));
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::journal::{AccountCode, JournalEntry, LedgerDocument};
    use crate::storage::json_store::save_year_to_dir;
    use crate::storage::sqlite_store::{get_sqlite_registered_years, load_year_from_sqlite};
    use chrono::NaiveDate;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("tax_return_mig_{nanos}"))
    }

    #[test]
    fn test_migration_from_json_to_sqlite() {
        let temp_dir = temp_test_dir();
        let json_dir = temp_dir.join("ledgers");
        let sqlite_file = temp_dir.join("tax_return.sqlite");

        // 2024年と2025年のJSONを作成
        let mut doc2024 = LedgerDocument::new(2024);
        doc2024.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2024, 10, 1).unwrap(),
            AccountCode::Cash,
            AccountCode::Sales,
            50_000,
            "2024売上",
        ));
        save_year_to_dir(&json_dir, &doc2024).unwrap();

        let mut doc2025 = LedgerDocument::new(2025);
        doc2025.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            120_000,
            "2025売上",
        ));
        doc2025.taxpayer.name = "移行テスト氏名".into();
        save_year_to_dir(&json_dir, &doc2025).unwrap();

        // 移行実行
        let report = migrate_json_directory_to_sqlite(&json_dir, &sqlite_file).unwrap();
        assert_eq!(report.migrated_years, vec![2024, 2025]);
        assert_eq!(report.total_entries, 2);

        // SQLiteから検証
        let conn = open_sqlite_connection(&sqlite_file).unwrap();
        let years = get_sqlite_registered_years(&conn).unwrap();
        assert_eq!(years, vec![2024, 2025]);

        let loaded2025 = load_year_from_sqlite(&conn, 2025).unwrap();
        assert_eq!(loaded2025.taxpayer.name, "移行テスト氏名");
        assert_eq!(loaded2025.entries.len(), 1);
        assert_eq!(loaded2025.entries[0].amount, 120_000);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
