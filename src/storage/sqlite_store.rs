//! GnuCash-Compatible SQLite Single-File Storage Engine.
//!
//! Consolidates all fiscal years, accounts, compound splits, and Japanese tax metadata
//! into a single SQLite database (`tax_return.sqlite` / `tax_return.gnucash`).
//!
//! Fully compliant with GnuCash's standard relational schema (`books`, `commodities`,
//! `accounts`, `transactions`, `splits`, `slots`) allowing direct inspection in GnuCash desktop,
//! backed by ACID transactions and B-Tree indexing.

use crate::core::journal::{
    AccountCode, AccountDefinition, AccountNature, AllocationRule, JournalEntry, LedgerDocument,
    TaxpayerInfo, CURRENT_SCHEMA_VERSION,
};
use crate::core::tax_calc::BlueSpecialDeductionKind;
use crate::storage::paths::{project_data_dir, PathError};
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SqliteError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("SQLiteデータベースエラー: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSONシリアライズエラー: {0}")]
    Json(#[from] serde_json::Error),
    #[error("入出力エラー: {0}")]
    Io(#[from] std::io::Error),
    #[error("データフォーマットエラー: {0}")]
    Format(String),
}

/// SQLite ストレージ全体の統計情報
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteStorageStats {
    pub db_path: PathBuf,
    pub file_size_bytes: u64,
    pub registered_years: Vec<i32>,
    pub total_transactions: usize,
    pub total_splits: usize,
    pub total_accounts: usize,
}

/// 標準の SQLite 単一データベースファイルパスを取得
pub fn sqlite_db_path() -> Result<PathBuf, SqliteError> {
    let dir = project_data_dir()?;
    Ok(dir.join("tax_return.sqlite"))
}

/// GnuCash 形式 GUID 文字列（32桁16進・ハイフンなし）を生成
fn gen_guid() -> String {
    Uuid::new_v4().simple().to_string()
}

/// GnuCash 標準テーブルおよび日本税務メタデータテーブルを初期化
pub fn init_sqlite_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        r#"
        -- GnuCash Books
        CREATE TABLE IF NOT EXISTS books (
            guid TEXT PRIMARY KEY,
            root_account_guid TEXT NOT NULL,
            root_template_guid TEXT
        );

        -- GnuCash Commodities (Currencies)
        CREATE TABLE IF NOT EXISTS commodities (
            guid TEXT PRIMARY KEY,
            namespace TEXT NOT NULL,
            mnemonic TEXT NOT NULL,
            fullname TEXT,
            alias_local TEXT,
            cusip TEXT,
            fraction INTEGER NOT NULL,
            quote_flag INTEGER NOT NULL,
            quote_source TEXT
        );

        -- GnuCash Accounts
        CREATE TABLE IF NOT EXISTS accounts (
            guid TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            account_type TEXT NOT NULL,
            commodity_guid TEXT,
            commodity_scu INTEGER NOT NULL DEFAULT 1,
            non_std_scu INTEGER NOT NULL DEFAULT 0,
            parent_guid TEXT,
            code TEXT,
            description TEXT,
            hidden INTEGER NOT NULL DEFAULT 0,
            placeholder INTEGER NOT NULL DEFAULT 0
        );

        -- GnuCash Transactions
        CREATE TABLE IF NOT EXISTS transactions (
            guid TEXT PRIMARY KEY,
            currency_guid TEXT NOT NULL,
            num TEXT,
            post_date TEXT NOT NULL,
            enter_date TEXT NOT NULL,
            description TEXT NOT NULL
        );

        -- GnuCash Splits
        CREATE TABLE IF NOT EXISTS splits (
            guid TEXT PRIMARY KEY,
            tx_guid TEXT NOT NULL,
            account_guid TEXT NOT NULL,
            memo TEXT,
            action TEXT,
            reconcile_state TEXT NOT NULL DEFAULT 'n',
            reconcile_date TEXT,
            value_num INTEGER NOT NULL,
            value_denom INTEGER NOT NULL,
            quantity_num INTEGER NOT NULL,
            quantity_denom INTEGER NOT NULL,
            lot_guid TEXT,
            FOREIGN KEY (tx_guid) REFERENCES transactions(guid) ON DELETE CASCADE,
            FOREIGN KEY (account_guid) REFERENCES accounts(guid)
        );

        -- GnuCash Slots (Key-Value Metadata)
        CREATE TABLE IF NOT EXISTS slots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            obj_guid TEXT NOT NULL,
            name TEXT NOT NULL,
            slot_type INTEGER NOT NULL,
            int64_val INTEGER,
            string_val TEXT,
            double_val REAL,
            timespec_val TEXT,
            guid_val TEXT,
            numeric_val_num INTEGER,
            numeric_val_denom INTEGER,
            gdate_val TEXT
        );

        -- 日本税務メタデータテーブル (年度ごとの納税者情報・控除・固定資産等を高速・完全保全)
        CREATE TABLE IF NOT EXISTS fiscal_year_meta (
            fiscal_year INTEGER PRIMARY KEY,
            taxpayer_json TEXT NOT NULL,
            special_deduction TEXT NOT NULL,
            accounts_json TEXT NOT NULL,
            allocation_rules_json TEXT NOT NULL,
            fixed_assets_json TEXT NOT NULL,
            deductions_json TEXT NOT NULL,
            auto_rules_json TEXT,
            consumption_tax_json TEXT,
            updated_at TEXT NOT NULL
        );

        -- インデックスの作成
        CREATE INDEX IF NOT EXISTS idx_transactions_post_date ON transactions(post_date);
        CREATE INDEX IF NOT EXISTS idx_splits_tx ON splits(tx_guid);
        CREATE INDEX IF NOT EXISTS idx_splits_account ON splits(account_guid);
        CREATE INDEX IF NOT EXISTS idx_accounts_name ON accounts(name);
        "#,
    )?;

    // 既存DBへのカラム追加マイグレーション
    let _ = conn.execute("ALTER TABLE fiscal_year_meta ADD COLUMN auto_rules_json TEXT", []);
    let _ = conn.execute("ALTER TABLE fiscal_year_meta ADD COLUMN consumption_tax_json TEXT", []);

    // JPY 通貨マスタとルートアカウントの初期化
    init_default_book_and_currency(conn)?;

    Ok(())
}

fn init_default_book_and_currency(conn: &Connection) -> Result<(), rusqlite::Error> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM commodities WHERE mnemonic = 'JPY'", [], |r| r.get(0))?;
    if count == 0 {
        let jpy_guid = "00000000000000000000000000000001";
        conn.execute(
            "INSERT INTO commodities (guid, namespace, mnemonic, fullname, fraction, quote_flag)
             VALUES (?1, 'CURRENCY', 'JPY', 'Japanese Yen', 1, 0)",
            params![jpy_guid],
        )?;

        let root_guid = "00000000000000000000000000000002";
        conn.execute(
            "INSERT INTO accounts (guid, name, account_type, commodity_guid, placeholder)
             VALUES (?1, 'Root Account', 'ROOT', ?2, 1)",
            params![root_guid, jpy_guid],
        )?;

        let book_guid = "00000000000000000000000000000003";
        conn.execute(
            "INSERT INTO books (guid, root_account_guid) VALUES (?1, ?2)",
            params![book_guid, root_guid],
        )?;
    }
    Ok(())
}

/// データベース接続を開き、スキーマを初期化
pub fn open_sqlite_connection(path: &Path) -> Result<Connection, SqliteError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    // WALモードと外部キー制約を有効化して高速化・整合性確保
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    init_sqlite_schema(&conn)?;
    Ok(conn)
}

/// 勘定科目および補助科目に対応する GnuCash Account GUID を取得（未登録なら自動作成）
fn ensure_account_guid(
    conn: &Connection,
    account_map: &mut HashMap<String, String>,
    code: &AccountCode,
    sub_account: Option<&str>,
) -> Result<String, rusqlite::Error> {
    let key = match sub_account {
        Some(s) if !s.trim().is_empty() => format!("{}:{}", code.label_ja(), s.trim()),
        _ => code.label_ja().to_string(),
    };

    if let Some(guid) = account_map.get(&key) {
        return Ok(guid.clone());
    }

    // データベースから既存のアカウントを検索
    let existing: Option<String> = conn
        .query_row(
            "SELECT guid FROM accounts WHERE name = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?;

    if let Some(guid) = existing {
        account_map.insert(key, guid.clone());
        return Ok(guid);
    }

    // 未登録なら作成
    let guid = gen_guid();
    let root_guid: String = conn.query_row("SELECT root_account_guid FROM books LIMIT 1", [], |r| r.get(0))?;
    let jpy_guid: String = conn.query_row("SELECT guid FROM commodities WHERE mnemonic = 'JPY' LIMIT 1", [], |r| r.get(0))?;

    let gnc_type = match code.nature() {
        AccountNature::Asset => match code {
            AccountCode::Bank => "BANK",
            AccountCode::Cash => "CASH",
            _ => "ASSET",
        },
        AccountNature::Liability => "LIABILITY",
        AccountNature::Equity => "EQUITY",
        AccountNature::Income => "INCOME",
        AccountNature::Expense => "EXPENSE",
    };

    conn.execute(
        "INSERT INTO accounts (guid, name, account_type, commodity_guid, parent_guid, code)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![guid, key, gnc_type, jpy_guid, root_guid, code.label_ja()],
    )?;

    account_map.insert(key, guid.clone());
    Ok(guid)
}

/// 単一年度の LedgerDocument を SQLite データベースに保存
pub fn save_year_to_sqlite(conn: &mut Connection, doc: &LedgerDocument) -> Result<(), SqliteError> {
    let tx = conn.transaction()?;

    let jpy_guid: String = tx.query_row("SELECT guid FROM commodities WHERE mnemonic = 'JPY' LIMIT 1", [], |r| r.get(0))?;

    // 1. 勘定科目キャッシュの作成
    let mut account_map: HashMap<String, String> = HashMap::new();

    // 2. 該当年度の既存トランザクションを特定して削除（その年度の仕訳をクリーン更新）
    let year_start = format!("{}-01-01", doc.fiscal_year);
    let year_end = format!("{}-12-31", doc.fiscal_year);

    tx.execute(
        "DELETE FROM transactions WHERE post_date >= ?1 AND post_date <= ?2",
        params![year_start, year_end],
    )?;

    // 3. 仕訳を transactions + splits に挿入
    for e in &doc.entries {
        let debit_guid = ensure_account_guid(&tx, &mut account_map, &e.debit, e.debit_sub.as_deref())?;
        let credit_guid = ensure_account_guid(&tx, &mut account_map, &e.credit, e.credit_sub.as_deref())?;

        let tx_guid = if !e.id.is_empty() && e.id.len() == 32 {
            e.id.clone()
        } else {
            gen_guid()
        };

        let date_str = e.date.format("%Y-%m-%d").to_string();
        let now_str = e.created_at.to_rfc3339();

        tx.execute(
            "INSERT INTO transactions (guid, currency_guid, post_date, enter_date, description)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![tx_guid, jpy_guid, date_str, now_str, e.description],
        )?;

        // 借方スプリット (正の金額)
        let split_debit_guid = gen_guid();
        tx.execute(
            "INSERT INTO splits (guid, tx_guid, account_guid, memo, value_num, value_denom, quantity_num, quantity_denom)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?5, 1)",
            params![split_debit_guid, tx_guid, debit_guid, e.debit_sub, e.amount],
        )?;

        // 貸方スプリット (負の金額)
        let split_credit_guid = gen_guid();
        tx.execute(
            "INSERT INTO splits (guid, tx_guid, account_guid, memo, value_num, value_denom, quantity_num, quantity_denom)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?5, 1)",
            params![split_credit_guid, tx_guid, credit_guid, e.credit_sub, -e.amount],
        )?;
    }

    // 4. 日本税務メタデータテーブルを更新
    let taxpayer_json = serde_json::to_string(&doc.taxpayer)?;
    let special_deduction = serde_json::to_string(&doc.special_deduction)?;
    let accounts_json = serde_json::to_string(&doc.accounts)?;
    let allocation_rules_json = serde_json::to_string(&doc.allocation_rules)?;
    let fixed_assets_json = serde_json::to_string(&doc.fixed_assets)?;
    let deductions_json = serde_json::to_string(&doc.deductions)?;
    let auto_rules_json = serde_json::to_string(&doc.auto_rules)?;
    let consumption_tax_json = serde_json::to_string(&doc.consumption_tax)?;
    let now = Local::now().to_rfc3339();

    tx.execute(
        r#"
        INSERT INTO fiscal_year_meta (
            fiscal_year, taxpayer_json, special_deduction, accounts_json,
            allocation_rules_json, fixed_assets_json, deductions_json, auto_rules_json, consumption_tax_json, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(fiscal_year) DO UPDATE SET
            taxpayer_json = excluded.taxpayer_json,
            special_deduction = excluded.special_deduction,
            accounts_json = excluded.accounts_json,
            allocation_rules_json = excluded.allocation_rules_json,
            fixed_assets_json = excluded.fixed_assets_json,
            deductions_json = excluded.deductions_json,
            auto_rules_json = excluded.auto_rules_json,
            consumption_tax_json = excluded.consumption_tax_json,
            updated_at = excluded.updated_at
        "#,
        params![
            doc.fiscal_year,
            taxpayer_json,
            special_deduction,
            accounts_json,
            allocation_rules_json,
            fixed_assets_json,
            deductions_json,
            auto_rules_json,
            consumption_tax_json,
            now
        ],
    )?;

    tx.commit()?;
    Ok(())
}

/// GnuCash アカウント名文字列から勘定科目と補助科目を解決
fn resolve_account_from_gnc_name(name: &str) -> (AccountCode, Option<String>) {
    let parts: Vec<&str> = name.split(':').collect();
    let code_name = parts[0];
    let sub = if parts.len() > 1 {
        Some(parts[1].to_string())
    } else {
        None
    };

    for standard in AccountCode::all() {
        if standard.label_ja() == code_name {
            return (standard.clone(), sub);
        }
    }

    (AccountCode::Custom(code_name.to_string()), sub)
}

/// SQLite データベースから指定年度の LedgerDocument を読み込み
pub fn load_year_from_sqlite(conn: &Connection, year: i32) -> Result<LedgerDocument, SqliteError> {
    // 1. メタデータテーブルを検索
    let meta_row: Option<(String, String, String, String, String, String, Option<String>, Option<String>)> = conn
        .query_row(
            r#"
            SELECT taxpayer_json, special_deduction, accounts_json,
                   allocation_rules_json, fixed_assets_json, deductions_json, auto_rules_json, consumption_tax_json
            FROM fiscal_year_meta
            WHERE fiscal_year = ?1
            "#,
            params![year],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6).ok(), r.get(7).ok())),
        )
        .optional()?;

    let (taxpayer, special_deduction, accounts, allocation_rules, fixed_assets, deductions, auto_rules, consumption_tax) =
        match meta_row {
            Some((tp, sd, accs, allocs, fa, deds, ar_opt, ct_opt)) => (
                serde_json::from_str::<TaxpayerInfo>(&tp).unwrap_or_default(),
                serde_json::from_str::<BlueSpecialDeductionKind>(&sd).unwrap_or_default(),
                serde_json::from_str::<Vec<AccountDefinition>>(&accs)
                    .unwrap_or_else(|_| AccountDefinition::default_registry()),
                serde_json::from_str::<Vec<AllocationRule>>(&allocs).unwrap_or_default(),
                serde_json::from_str::<Vec<crate::core::depreciation::FixedAsset>>(&fa)
                    .unwrap_or_default(),
                serde_json::from_str::<crate::core::deductions::TaxDeductions>(&deds)
                    .unwrap_or_default(),
                ar_opt
                    .and_then(|json| serde_json::from_str::<Vec<crate::core::auto_rule::AutoBookingRule>>(&json).ok())
                    .unwrap_or_else(crate::core::auto_rule::default_auto_rules),
                ct_opt
                    .and_then(|json| serde_json::from_str::<crate::core::consumption_tax::ConsumptionTaxConfig>(&json).ok())
                    .unwrap_or_default(),
            ),
            None => (
                TaxpayerInfo::default(),
                BlueSpecialDeductionKind::default(),
                AccountDefinition::default_registry(),
                Vec::new(),
                Vec::new(),
                Default::default(),
                crate::core::auto_rule::default_auto_rules(),
                Default::default(),
            ),
        };

    // 2. 指定年度の transactions と splits を取得
    let year_start = format!("{year}-01-01");
    let year_end = format!("{year}-12-31");

    let mut stmt = conn.prepare(
        r#"
        SELECT t.guid, t.post_date, t.enter_date, t.description,
               s.guid, a.name, s.value_num, s.memo
        FROM transactions t
        JOIN splits s ON t.guid = s.tx_guid
        JOIN accounts a ON s.account_guid = a.guid
        WHERE t.post_date >= ?1 AND t.post_date <= ?2
        ORDER BY t.post_date ASC, t.enter_date ASC
        "#,
    )?;

    struct RawSplitRow {
        tx_guid: String,
        post_date: String,
        enter_date: String,
        description: String,
        account_name: String,
        value_num: i64,
        memo: Option<String>,
    }

    let rows = stmt.query_map(params![year_start, year_end], |r| {
        Ok(RawSplitRow {
            tx_guid: r.get(0)?,
            post_date: r.get(1)?,
            enter_date: r.get(2)?,
            description: r.get(3)?,
            account_name: r.get(5)?,
            value_num: r.get(6)?,
            memo: r.get(7)?,
        })
    })?;

    // トランザクションGUIDごとにスプリットをグルーピング
    let mut tx_splits: HashMap<String, Vec<RawSplitRow>> = HashMap::new();
    let mut tx_order: Vec<String> = Vec::new();

    for row in rows {
        let r = row?;
        if !tx_splits.contains_key(&r.tx_guid) {
            tx_order.push(r.tx_guid.clone());
        }
        tx_splits.entry(r.tx_guid.clone()).or_default().push(r);
    }

    let mut entries = Vec::new();

    for tx_guid in tx_order {
        if let Some(splits) = tx_splits.get(&tx_guid) {
            let debits: Vec<&RawSplitRow> = splits.iter().filter(|s| s.value_num > 0).collect();
            let credits: Vec<&RawSplitRow> = splits.iter().filter(|s| s.value_num < 0).collect();

            if debits.is_empty() || credits.is_empty() {
                continue;
            }

            let first = &splits[0];
            let date = NaiveDate::parse_from_str(&first.post_date, "%Y-%m-%d")
                .unwrap_or_else(|_| NaiveDate::from_ymd_opt(year, 1, 1).unwrap());
            let created_at = chrono::DateTime::parse_from_rfc3339(&first.enter_date)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(|_| Local::now());

            if debits.len() == 1 && credits.len() == 1 {
                let d = debits[0];
                let c = credits[0];
                let (debit_code, debit_sub) = resolve_account_from_gnc_name(&d.account_name);
                let (credit_code, credit_sub) = resolve_account_from_gnc_name(&c.account_name);

                entries.push(JournalEntry {
                    id: tx_guid.clone(),
                    date,
                    debit: debit_code,
                    debit_sub: debit_sub.or_else(|| d.memo.clone()),
                    credit: credit_code,
                    credit_sub: credit_sub.or_else(|| c.memo.clone()),
                    amount: d.value_num,
                    description: first.description.clone(),
                    created_at,
                });
            } else {
                // 複合取引の展開
                for d in &debits {
                    for c in &credits {
                        let amt = d.value_num.min(-c.value_num);
                        if amt > 0 {
                            let (debit_code, debit_sub) = resolve_account_from_gnc_name(&d.account_name);
                            let (credit_code, credit_sub) = resolve_account_from_gnc_name(&c.account_name);
                            entries.push(JournalEntry {
                                id: Uuid::new_v4().to_string(),
                                date,
                                debit: debit_code,
                                debit_sub: debit_sub.or_else(|| d.memo.clone()),
                                credit: credit_code,
                                credit_sub: credit_sub.or_else(|| c.memo.clone()),
                                amount: amt,
                                description: first.description.clone(),
                                created_at,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(LedgerDocument {
        version: CURRENT_SCHEMA_VERSION,
        fiscal_year: year,
        entries,
        taxpayer,
        special_deduction,
        accounts,
        allocation_rules,
        fixed_assets,
        deductions,
        auto_rules,
        consumption_tax,
    })
}

/// SQLite に登録されているすべての年度一覧を取得
pub fn get_sqlite_registered_years(conn: &Connection) -> Result<Vec<i32>, SqliteError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT DISTINCT fiscal_year FROM (
            SELECT fiscal_year FROM fiscal_year_meta
            UNION
            SELECT CAST(SUBSTR(post_date, 1, 4) AS INTEGER) AS fiscal_year FROM transactions
        ) WHERE fiscal_year > 1900
        ORDER BY fiscal_year ASC
        "#,
    )?;

    let rows = stmt.query_map([], |r| r.get(0))?;
    let mut years = Vec::new();
    for r in rows {
        years.push(r?);
    }
    Ok(years)
}

/// SQLite データベースの全体統計を取得
pub fn get_sqlite_stats(path: &Path, conn: &Connection) -> Result<SqliteStorageStats, SqliteError> {
    let file_size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let registered_years = get_sqlite_registered_years(conn)?;

    let total_transactions: usize = conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?;
    let total_splits: usize = conn.query_row("SELECT COUNT(*) FROM splits", [], |r| r.get(0))?;
    let total_accounts: usize = conn.query_row("SELECT COUNT(*) FROM accounts WHERE account_type != 'ROOT'", [], |r| r.get(0))?;

    Ok(SqliteStorageStats {
        db_path: path.to_path_buf(),
        file_size_bytes,
        registered_years,
        total_transactions,
        total_splits,
        total_accounts,
    })
}

/// GnuCash 形式の `.gnucash` 単一ファイルとして指定パスへバックアップ・エクスポート
pub fn export_gnucash_database(conn: &Connection, target_path: &Path) -> Result<(), SqliteError> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    conn.execute("VACUUM INTO ?1", params![target_path.to_string_lossy()])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_init_and_roundtrip() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_sqlite_schema(&conn).unwrap();

        let mut doc = LedgerDocument::new(2025);
        doc.entries.push(JournalEntry::new_with_sub(
            NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
            AccountCode::Bank,
            Some("三井住友銀行".into()),
            AccountCode::Sales,
            None,
            250_000,
            "売上入金",
        ));
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 4, 10).unwrap(),
            AccountCode::Rent,
            AccountCode::Bank,
            80_000,
            "4月分家賃",
        ));
        doc.taxpayer.name = "テスト太郎".into();
        doc.special_deduction = BlueSpecialDeductionKind::Yen650k;

        save_year_to_sqlite(&mut conn, &doc).unwrap();

        // 読み込み
        let loaded = load_year_from_sqlite(&conn, 2025).unwrap();
        assert_eq!(loaded.fiscal_year, 2025);
        assert_eq!(loaded.taxpayer.name, "テスト太郎");
        assert_eq!(loaded.special_deduction, BlueSpecialDeductionKind::Yen650k);
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].amount, 250_000);
        assert_eq!(loaded.entries[0].debit, AccountCode::Bank);
        assert_eq!(loaded.entries[0].debit_sub.as_deref(), Some("三井住友銀行"));
        assert_eq!(loaded.entries[0].credit, AccountCode::Sales);
        assert_eq!(loaded.entries[1].amount, 80_000);
        assert_eq!(loaded.entries[1].debit, AccountCode::Rent);

        // 年度一覧
        let years = get_sqlite_registered_years(&conn).unwrap();
        assert_eq!(years, vec![2025]);

        // GnuCash標準テーブルの整合性検証
        let tx_count: i64 = conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0)).unwrap();
        assert_eq!(tx_count, 2);
        let split_count: i64 = conn.query_row("SELECT COUNT(*) FROM splits", [], |r| r.get(0)).unwrap();
        assert_eq!(split_count, 4);

        // スプリット合計（貸借バランス）が常に 0 であること
        let balance_sum: i64 = conn.query_row("SELECT SUM(value_num) FROM splits", [], |r| r.get(0)).unwrap();
        assert_eq!(balance_sum, 0);
    }

    #[test]
    fn test_multi_year_isolation() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_sqlite_schema(&conn).unwrap();

        let mut doc2024 = LedgerDocument::new(2024);
        doc2024.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            AccountCode::Cash,
            AccountCode::Sales,
            10_000,
            "2024年売上",
        ));

        let mut doc2025 = LedgerDocument::new(2025);
        doc2025.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            20_000,
            "2025年売上",
        ));

        save_year_to_sqlite(&mut conn, &doc2024).unwrap();
        save_year_to_sqlite(&mut conn, &doc2025).unwrap();

        let years = get_sqlite_registered_years(&conn).unwrap();
        assert_eq!(years, vec![2024, 2025]);

        let loaded2024 = load_year_from_sqlite(&conn, 2024).unwrap();
        assert_eq!(loaded2024.entries.len(), 1);
        assert_eq!(loaded2024.entries[0].amount, 10_000);

        let loaded2025 = load_year_from_sqlite(&conn, 2025).unwrap();
        assert_eq!(loaded2025.entries.len(), 1);
        assert_eq!(loaded2025.entries[0].amount, 20_000);
    }
}
