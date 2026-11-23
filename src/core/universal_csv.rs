//! Universal Bank and Credit Card CSV Importer with Column Mapping and Auto-Classification.
//!
//! Supports diverse financial institution CSV export formats (MUFG, SMBC, Mizuho,
//! Yuucho, Rakuten Bank, Rakuten Card, SMBC Card, etc.) with configurable column mapping
//! and rule-based automated transaction categorization.

use crate::core::auto_rule::{match_auto_rule, AutoBookingRule};
use crate::core::csv_import::{decode_csv_bytes, parse_amount, parse_flexible_date, CsvRowError};
use crate::core::journal::{AccountCode, JournalEntry};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 金額列の構成方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CsvAmountMode {
    /// 入出金別2列（例: 銀行明細「お引き出し」「お預け入れ」）
    DualColumn {
        withdrawal_col: usize,
        deposit_col: usize,
    },
    /// 金額1列（例: クレジットカード利用金額、または符号付き1列）
    SingleColumn {
        amount_col: usize,
        /// マイナス値または特定記号が出金（支出）を表すか
        negative_is_expense: bool,
    },
}

/// 汎用CSV列マッピング設定
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvMappingConfig {
    /// プリセット名称（例: "銀行（入出金2列）", "クレジットカード（利用明細）"）
    pub preset_name: String,
    /// ヘッダー行インデックス（0開始、標準は0行目または1行目）
    pub header_row_index: usize,
    /// 取引日付の列インデックス
    pub date_col: usize,
    /// 摘要・取引内容・利用店名の列インデックス
    pub description_col: usize,
    /// 金額の列構成
    pub amount_mode: CsvAmountMode,
    /// 取引の基礎口座（銀行CSVなら普通預金、カードCSVなら未払金）
    pub base_account: AccountCode,
    /// 基礎口座の補助科目（例: "楽天銀行", "三菱UFJ", "楽天カード"）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_sub_account: Option<String>,
}

impl Default for CsvMappingConfig {
    fn default() -> Self {
        Self::preset_bank_dual()
    }
}

impl CsvMappingConfig {
    /// プリセット: 銀行明細（入出金2列形式: 日付, 摘要, 出金額, 入金額）
    pub fn preset_bank_dual() -> Self {
        Self {
            preset_name: "銀行（出金・入金 2列形式）".into(),
            header_row_index: 0,
            date_col: 0,
            description_col: 1,
            amount_mode: CsvAmountMode::DualColumn {
                withdrawal_col: 2,
                deposit_col: 3,
            },
            base_account: AccountCode::Bank,
            base_sub_account: None,
        }
    }

    /// プリセット: 銀行明細（金額1列・正負符号形式: 日付, 摘要, 取引金額）
    pub fn preset_bank_single() -> Self {
        Self {
            preset_name: "銀行（取引金額 1列形式）".into(),
            header_row_index: 0,
            date_col: 0,
            description_col: 1,
            amount_mode: CsvAmountMode::SingleColumn {
                amount_col: 2,
                negative_is_expense: true,
            },
            base_account: AccountCode::Bank,
            base_sub_account: None,
        }
    }

    /// プリセット: クレジットカード明細（利用日, 利用店名・摘要, 利用金額）
    pub fn preset_credit_card() -> Self {
        Self {
            preset_name: "クレジットカード（利用明細形式）".into(),
            header_row_index: 0,
            date_col: 0,
            description_col: 1,
            amount_mode: CsvAmountMode::SingleColumn {
                amount_col: 2,
                negative_is_expense: false, // カード明細の利用額は正の数値で支出
            },
            base_account: AccountCode::AccountsPayable,
            base_sub_account: None,
        }
    }

    pub fn all_presets() -> Vec<Self> {
        vec![
            Self::preset_bank_dual(),
            Self::preset_bank_single(),
            Self::preset_credit_card(),
        ]
    }
}

/// 解析・自動分類された1行の仕訳プレビュー情報
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UniversalCsvRow {
    pub line_number: usize,
    pub date: NaiveDate,
    pub description: String,
    pub amount: i64,
    /// 支出（true）または 収入（false）
    pub is_expense: bool,
    /// 合致した自動仕訳ルール名（合致しない場合は None）
    pub matched_rule_name: Option<String>,
    pub debit: AccountCode,
    pub debit_sub: Option<String>,
    pub credit: AccountCode,
    pub credit_sub: Option<String>,
}

/// 汎用CSVインポート結果レポート
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UniversalCsvReport {
    /// 生成された正規仕訳リスト
    pub entries: Vec<JournalEntry>,
    /// プレビュー詳細行リスト
    pub rows: Vec<UniversalCsvRow>,
    /// 解析エラー行
    pub errors: Vec<CsvRowError>,
    /// 収入総額（入金合計）
    pub total_income: i64,
    /// 支出総額（出金合計）
    pub total_expense: i64,
    /// 自動仕訳ルールに合致して自動分類された件数
    pub matched_rules_count: usize,
    /// 総読み込み行数
    pub total_rows_inspected: usize,
}

/// 汎用CSVデータをマッピング設定と自動仕訳ルールに基づいて解析・仕訳生成
pub fn parse_universal_csv(
    raw_bytes: &[u8],
    config: &CsvMappingConfig,
    fiscal_year: i32,
    rules: &[AutoBookingRule],
) -> UniversalCsvReport {
    let text = decode_csv_bytes(raw_bytes);
    parse_universal_csv_str(&text, config, fiscal_year, rules)
}

/// CSV文字列から汎用仕訳データを解析
pub fn parse_universal_csv_str(
    text: &str,
    config: &CsvMappingConfig,
    fiscal_year: i32,
    rules: &[AutoBookingRule],
) -> UniversalCsvReport {
    let mut report = UniversalCsvReport::default();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());

    for (row_idx, result) in rdr.records().enumerate() {
        report.total_rows_inspected += 1;
        let line_number = row_idx + 1;

        // ヘッダー行およびそれ以前の行はスキップ
        if row_idx <= config.header_row_index {
            continue;
        }

        let record = match result {
            Ok(rec) => rec,
            Err(e) => {
                report.errors.push(CsvRowError {
                    row_index: row_idx,
                    line_number,
                    message: format!("CSV構文エラー: {e}"),
                    raw_data: String::new(),
                });
                continue;
            }
        };

        if record.is_empty() || record.iter().all(|f| f.trim().is_empty()) {
            continue;
        }

        // 1. 日付の取得とパース
        let date_str = record.get(config.date_col).unwrap_or("").trim();
        let date = match parse_flexible_date(date_str, fiscal_year) {
            Ok(d) => d,
            Err(e) => {
                report.errors.push(CsvRowError {
                    row_index: row_idx,
                    line_number,
                    message: format!("日付パースエラー: {e}"),
                    raw_data: format!("{:?}", record),
                });
                continue;
            }
        };

        // 2. 摘要・取引先
        let desc = record.get(config.description_col).unwrap_or("").trim().to_string();
        let desc_clean = if desc.is_empty() { "（摘要なし）".to_string() } else { desc };

        // 3. 金額と出金/入金判定
        let (amount, is_expense) = match config.amount_mode {
            CsvAmountMode::DualColumn { withdrawal_col, deposit_col } => {
                let with_str = record.get(withdrawal_col).unwrap_or("").trim();
                let dep_str = record.get(deposit_col).unwrap_or("").trim();

                let with_val = parse_amount(with_str).unwrap_or(0);
                let dep_val = parse_amount(dep_str).unwrap_or(0);

                if with_val > 0 {
                    (with_val, true)
                } else if dep_val > 0 {
                    (dep_val, false)
                } else {
                    // 両方空または0
                    continue;
                }
            }
            CsvAmountMode::SingleColumn { amount_col, negative_is_expense } => {
                let amt_str = record.get(amount_col).unwrap_or("").trim();
                let is_neg = amt_str.starts_with('-') || amt_str.starts_with('▲') || amt_str.starts_with('△');
                let val = match parse_amount(amt_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let is_expense = if negative_is_expense {
                    is_neg
                } else {
                    !is_neg
                };
                (val, is_expense)
            }
        };

        if amount <= 0 {
            continue;
        }

        // 4. ルールベース自動仕訳の判定
        let matched = match_auto_rule(&desc_clean, rules);
        let matched_rule_name = matched.map(|r| r.name.clone());

        let target_account = if let Some(rule) = matched {
            rule.target_account.clone()
        } else if is_expense {
            // 出金で合致ルールがない場合は雑費（後でユーザーが自由変更可能）
            AccountCode::Miscellaneous
        } else {
            // 入金で合致ルールがない場合は売上
            AccountCode::Sales
        };

        let target_sub = matched.and_then(|r| r.target_sub_account.clone());

        let (debit, debit_sub, credit, credit_sub) = if is_expense {
            report.total_expense += amount;
            // 支出: 借方 = 費用科目 / 貸方 = 基礎口座 (銀行またはカード未払金)
            (
                target_account,
                target_sub,
                config.base_account.clone(),
                config.base_sub_account.clone(),
            )
        } else {
            report.total_income += amount;
            // 収入: 借方 = 基礎口座 (銀行) / 貸方 = 収益科目 (売上等)
            (
                config.base_account.clone(),
                config.base_sub_account.clone(),
                target_account,
                target_sub,
            )
        };

        if matched_rule_name.is_some() {
            report.matched_rules_count += 1;
        }

        let entry = JournalEntry {
            id: uuid::Uuid::new_v4().to_string(),
            date,
            debit: debit.clone(),
            debit_sub: debit_sub.clone(),
            credit: credit.clone(),
            credit_sub: credit_sub.clone(),
            amount,
            description: desc_clean.clone(),
            created_at: chrono::Local::now(),
        };

        report.rows.push(UniversalCsvRow {
            line_number,
            date,
            description: desc_clean,
            amount,
            is_expense,
            matched_rule_name,
            debit,
            debit_sub,
            credit,
            credit_sub,
        });

        report.entries.push(entry);
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::auto_rule::default_auto_rules;

    #[test]
    fn test_parse_universal_bank_dual_column() {
        let csv = "年月日,内容,出金金額,入金金額,残高\n\
2025/04/10,Amazon利用代金,15800,,1200000\n\
2025/04/15,クライアント売上振込,,330000,1530000\n\
2025/04/20,セブン-イレブン渋谷,850,,1529150\n";

        let mut config = CsvMappingConfig::preset_bank_dual();
        config.base_sub_account = Some("楽天銀行".into());
        let rules = default_auto_rules();

        let report = parse_universal_csv_str(csv, &config, 2025, &rules);
        assert_eq!(report.entries.len(), 3);
        assert_eq!(report.total_expense, 15_800 + 850);
        assert_eq!(report.total_income, 330_000);
        assert_eq!(report.matched_rules_count, 2); // Amazon + セブン

        // 1行目: Amazon 消耗品費
        let e1 = &report.entries[0];
        assert_eq!(e1.debit, AccountCode::Supplies);
        assert_eq!(e1.credit, AccountCode::Bank);
        assert_eq!(e1.credit_sub.as_deref(), Some("楽天銀行"));
        assert_eq!(e1.amount, 15_800);

        // 2行目: 売上入金
        let e2 = &report.entries[1];
        assert_eq!(e2.debit, AccountCode::Bank);
        assert_eq!(e2.credit, AccountCode::Sales);
        assert_eq!(e2.amount, 330_000);

        // 3行目: セブン-イレブン 雑費
        let e3 = &report.entries[2];
        assert_eq!(e3.debit, AccountCode::Miscellaneous);
        assert_eq!(e3.amount, 850);
    }

    #[test]
    fn test_parse_credit_card_single_column() {
        let csv = "利用日,利用先・店名,利用金額\n\
2025/05/01,モバイルSuicaチャージ,5000\n\
2025/05/05,Google Cloud Platform,12500\n";

        let mut config = CsvMappingConfig::preset_credit_card();
        config.base_sub_account = Some("楽天カード".into());
        let rules = default_auto_rules();

        let report = parse_universal_csv_str(csv, &config, 2025, &rules);
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.matched_rules_count, 2);

        // Suica -> Travel, Credit -> AccountsPayable
        let e1 = &report.entries[0];
        assert_eq!(e1.debit, AccountCode::Travel);
        assert_eq!(e1.credit, AccountCode::AccountsPayable);
        assert_eq!(e1.credit_sub.as_deref(), Some("楽天カード"));

        // Google -> Communication, Credit -> AccountsPayable
        let e2 = &report.entries[1];
        assert_eq!(e2.debit, AccountCode::Communication);
        assert_eq!(e2.credit, AccountCode::AccountsPayable);
    }
}
