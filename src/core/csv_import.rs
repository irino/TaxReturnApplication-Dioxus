//! CSV import for legacy 2-row header accounting exports.
//! Pure accounting and parsing logic — no UI dependencies.

use crate::core::journal::{AccountCode, JournalEntry};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Detailed error for a specific row in the CSV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvRowError {
    pub row_index: usize,
    pub line_number: usize,
    pub message: String,
    pub raw_data: String,
}

/// Result report returned after parsing a legacy CSV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CsvImportReport {
    /// Successfully imported journal entries.
    pub entries: Vec<JournalEntry>,
    /// Warnings (e.g. unknown account fallback notifications).
    pub warnings: Vec<String>,
    /// Row-level errors for rows that could not be imported.
    pub errors: Vec<CsvRowError>,
    /// Total data rows inspected (excluding header rows).
    pub total_data_rows: usize,
    /// Number of rows skipped (e.g. empty rows).
    pub skipped_rows: usize,
}

impl CsvImportReport {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.errors.is_empty()
    }

    pub fn total_amount(&self) -> i64 {
        self.entries.iter().map(|e| e.amount).sum()
    }
}

/// Decode raw bytes to string, automatically handling UTF-8 and Shift_JIS (CP932).
pub fn decode_csv_bytes(raw_bytes: &[u8]) -> String {
    // If it begins with UTF-8 BOM, strip it
    let bytes = if raw_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &raw_bytes[3..]
    } else {
        raw_bytes
    };

    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _encoding_used, _had_errors) = encoding_rs::SHIFT_JIS.decode(bytes);
            cow.into_owned()
        }
    }
}

/// Normalize Japanese full-width digits and spaces.
fn normalize_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '０'..='９' => ((c as u32 - '０' as u32) as u8 + b'0') as char,
            '　' => ' ',
            _ => c,
        })
        .collect()
}

/// Parse date string in various formats:
/// - "1月5日", "01月05日", "１月５日"
/// - "2026/01/05", "2026-01-05", "2026.01.05"
/// - "01/05", "1/5"
pub fn parse_flexible_date(raw: &str, fiscal_year: i32) -> Result<NaiveDate, String> {
    let text = normalize_digits(raw.trim());
    if text.is_empty() {
        return Err("日付が空です".to_string());
    }

    // Pattern 1: YYYY-MM-DD, YYYY/MM/DD, YYYY.MM.DD
    for fmt in &["%Y-%m-%d", "%Y/%m/%d", "%Y.%m.%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(&text, fmt) {
            return Ok(d);
        }
    }

    // Pattern 2: "1月5日"
    if text.contains('月') {
        if let Some((m_part, rest)) = text.split_once('月') {
            let d_part = rest.trim_end_matches('日').trim();
            if let (Ok(m), Ok(d)) = (m_part.trim().parse::<u32>(), d_part.parse::<u32>()) {
                if let Some(date) = NaiveDate::from_ymd_opt(fiscal_year, m, d) {
                    return Ok(date);
                }
            }
        }
    }

    // Pattern 3: "1/5", "01/05"
    if text.contains('/') {
        let parts: Vec<&str> = text.split('/').collect();
        if parts.len() == 2 {
            if let (Ok(m), Ok(d)) = (parts[0].trim().parse::<u32>(), parts[1].trim().parse::<u32>()) {
                if let Some(date) = NaiveDate::from_ymd_opt(fiscal_year, m, d) {
                    return Ok(date);
                }
            }
        }
    }

    Err(format!("日付形式を解釈できませんでした: '{raw}'"))
}

/// Parse yen amount from formatted strings like `"165,000.00"`, `"165000"`, `¥165,000`.
pub fn parse_amount(raw: &str) -> Result<i64, String> {
    let clean: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',' && *c != '"' && *c != '¥' && *c != '￥')
        .collect();

    if clean.is_empty() {
        return Err("金額が空です".to_string());
    }

    // Check if there is a decimal part like .00
    if let Ok(val) = clean.parse::<f64>() {
        let rounded = val.round() as i64;
        if rounded <= 0 {
            return Err(format!("金額は1円以上である必要があります: '{raw}'"));
        }
        return Ok(rounded);
    }

    Err(format!("金額を数値として解釈できませんでした: '{raw}'"))
}

/// Map account code number or account name to `AccountCode`.
pub fn map_account(code_str: &str, name_str: &str, is_credit: bool) -> (AccountCode, Option<String>) {
    let code_trimmed = code_str.trim();
    let name_trimmed = name_str.trim();

    // 1. Direct match with standard Japanese labels
    for code in AccountCode::all() {
        if code.label_ja() == name_trimmed {
            return (code.clone(), None);
        }
    }

    // 2. Known account code matches
    match code_trimmed {
        "1" => return (AccountCode::Sales, None),
        "2" | "3" => return (AccountCode::Sales, None),
        "101" => return (AccountCode::Cash, None),
        "102" | "103" | "104" | "105" => return (AccountCode::Bank, None),
        "111" | "112" => return (AccountCode::AccountsReceivable, None),
        "113" => return (AccountCode::Cash, None),
        "201" | "202" | "211" | "212" | "213" | "214" | "215" => {
            return (AccountCode::AccountsPayable, None)
        }
        "301" => return (AccountCode::Capital, None),
        "302" => return (AccountCode::Drawings, None),
        "303" => return (AccountCode::OwnerContribution, None),
        "401" => return (AccountCode::CostOfSales, None),
        "501" => return (AccountCode::CostOfSales, None),
        "502" | "510" => return (AccountCode::Supplies, None),
        "503" => return (AccountCode::Utilities, None),
        "504" => return (AccountCode::Travel, None),
        "505" => return (AccountCode::Communication, None),
        "515" => return (AccountCode::Rent, None),
        "506" | "507" | "508" | "509" | "511" | "512" | "513" | "514" | "516" | "517" | "518"
        | "519" => return (AccountCode::Miscellaneous, None),
        _ => {}
    }

    // 3. Name substring heuristics
    if name_trimmed.contains("預金") || name_trimmed.contains("銀行") {
        return (AccountCode::Bank, None);
    }
    if name_trimmed.contains("現金") {
        return (AccountCode::Cash, None);
    }
    if name_trimmed.contains("売掛") || name_trimmed.contains("未収") {
        return (AccountCode::AccountsReceivable, None);
    }
    if name_trimmed.contains("買掛") || name_trimmed.contains("未払") {
        return (AccountCode::AccountsPayable, None);
    }
    if name_trimmed.contains("売上")
        || name_trimmed.contains("収入")
        || name_trimmed.contains("賃料")
        || name_trimmed.contains("家賃収入")
    {
        return (AccountCode::Sales, None);
    }
    if name_trimmed.contains("仕入") {
        return (AccountCode::CostOfSales, None);
    }
    if name_trimmed.contains("交通") || name_trimmed.contains("旅費") {
        return (AccountCode::Travel, None);
    }
    if name_trimmed.contains("通信") || name_trimmed.contains("電話") {
        return (AccountCode::Communication, None);
    }
    if name_trimmed.contains("消耗品") || name_trimmed.contains("事務") {
        return (AccountCode::Supplies, None);
    }
    if name_trimmed.contains("地代") || name_trimmed.contains("家賃") {
        return (AccountCode::Rent, None);
    }
    if name_trimmed.contains("水道")
        || name_trimmed.contains("光熱")
        || name_trimmed.contains("電気")
        || name_trimmed.contains("ガス")
    {
        return (AccountCode::Utilities, None);
    }
    if name_trimmed.contains("事業主貸") {
        return (AccountCode::Drawings, None);
    }
    if name_trimmed.contains("事業主借") {
        return (AccountCode::OwnerContribution, None);
    }
    if name_trimmed.contains("元入金") || name_trimmed.contains("資本") {
        return (AccountCode::Capital, None);
    }
    if name_trimmed.contains("広告") || name_trimmed.contains("宣伝") {
        return (AccountCode::Advertising, None);
    }
    if name_trimmed.contains("接待") || name_trimmed.contains("交際") {
        return (AccountCode::Entertainment, None);
    }
    if name_trimmed.contains("保険") {
        return (AccountCode::Insurance, None);
    }
    if name_trimmed.contains("修繕") || name_trimmed.contains("修理") {
        return (AccountCode::Repair, None);
    }
    if name_trimmed.contains("外注") || name_trimmed.contains("委託") {
        return (AccountCode::Subcontracting, None);
    }
    if name_trimmed.contains("給料") || name_trimmed.contains("給与") || name_trimmed.contains("賃金") {
        return (AccountCode::Salaries, None);
    }
    if name_trimmed.contains("福利") {
        return (AccountCode::Welfare, None);
    }
    if name_trimmed.contains("利子") || name_trimmed.contains("利息") {
        return (AccountCode::Interest, None);
    }
    if name_trimmed.contains("租税") || name_trimmed.contains("公課") || name_trimmed.contains("印紙") {
        return (AccountCode::TaxesAndDues, None);
    }
    if name_trimmed.contains("荷造") || name_trimmed.contains("運賃") || name_trimmed.contains("送料") {
        return (AccountCode::PackingShipping, None);
    }

    // Fallback with warning
    let fallback = if is_credit {
        AccountCode::Sales
    } else {
        AccountCode::Miscellaneous
    };
    let warn = format!(
        "科目「{}」(コード: {}) は未対応のため「{}」に代替しました",
        if name_trimmed.is_empty() { "(名称なし)" } else { name_trimmed },
        if code_trimmed.is_empty() { "-" } else { code_trimmed },
        fallback.label_ja()
    );
    (fallback, Some(warn))
}

/// Parse legacy 2-row header CSV string and convert into `CsvImportReport`.
pub fn parse_legacy_csv_str(csv_str: &str, fiscal_year: i32) -> CsvImportReport {
    let mut report = CsvImportReport::default();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_str.as_bytes());

    let mut row_idx: usize = 0;

    for result in reader.records() {
        row_idx += 1;
        let record = match result {
            Ok(rec) => rec,
            Err(e) => {
                report.errors.push(CsvRowError {
                    row_index: row_idx,
                    line_number: row_idx,
                    message: format!("CSV行の読み込みに失敗しました: {e}"),
                    raw_data: String::new(),
                });
                continue;
            }
        };

        // Skip completely empty rows
        if record.is_empty() || record.iter().all(|f| f.trim().is_empty()) {
            report.skipped_rows += 1;
            continue;
        }

        // Detect and skip header rows (first 2 rows typically)
        let first_col = record.get(0).map(|s| s.trim()).unwrap_or_default();
        let second_col = record.get(1).map(|s| s.trim()).unwrap_or_default();
        if first_col == "日付"
            || second_col == "№"
            || second_col == "No"
            || record.iter().any(|f| f.contains("借　　　　方") || f.contains("科目ｺｰﾄﾞ"))
        {
            // Header row, safely ignore
            continue;
        }

        report.total_data_rows += 1;

        // Column mapping according to the specified 2-row header format:
        // 0: 日付
        // 1: №
        // 2: 借方 科目ｺｰﾄﾞ
        // 3: 借方 勘定科目
        // 4: 借方 金額
        // 5: 貸方 科目ｺｰﾄﾞ
        // 6: 貸方 勘定科目
        // 7: 貸方 金額
        // 8: 月
        // 9: 重複チェック
        // 10: 集計科目コード１
        // 11: 集計科目コード２
        // 12: 集計除外摘要文１
        // 13: 集計除外摘要文２
        // 14: 摘要
        // 15: 摘要２
        // 16: 摘要３

        let raw_line = record.iter().collect::<Vec<_>>().join(",");

        // 1. Date
        let date_str = match record.get(0) {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => {
                report.errors.push(CsvRowError {
                    row_index: report.total_data_rows,
                    line_number: row_idx,
                    message: "日付列が空です".to_string(),
                    raw_data: raw_line,
                });
                continue;
            }
        };

        let date = match parse_flexible_date(date_str, fiscal_year) {
            Ok(d) => d,
            Err(err) => {
                report.errors.push(CsvRowError {
                    row_index: report.total_data_rows,
                    line_number: row_idx,
                    message: err,
                    raw_data: raw_line,
                });
                continue;
            }
        };

        // 2. Amount (try debit amount at col 4, then credit amount at col 7)
        let debit_amount_str = record.get(4).map(|s| s.trim()).unwrap_or_default();
        let credit_amount_str = record.get(7).map(|s| s.trim()).unwrap_or_default();

        let amount = match (parse_amount(debit_amount_str), parse_amount(credit_amount_str)) {
            (Ok(a), _) => a,
            (_, Ok(a)) => a,
            (Err(e1), Err(_)) => {
                report.errors.push(CsvRowError {
                    row_index: report.total_data_rows,
                    line_number: row_idx,
                    message: format!("金額の取得に失敗しました: {e1}"),
                    raw_data: raw_line,
                });
                continue;
            }
        };

        // 3. Accounts
        let debit_code = record.get(2).unwrap_or("");
        let debit_name = record.get(3).unwrap_or("");
        let credit_code = record.get(5).unwrap_or("");
        let credit_name = record.get(6).unwrap_or("");

        let (debit_account, debit_warn) = map_account(debit_code, debit_name, false);
        let (credit_account, credit_warn) = map_account(credit_code, credit_name, true);

        if let Some(w) = debit_warn {
            let msg = format!("{}行目(借方): {}", row_idx, w);
            if !report.warnings.contains(&msg) {
                report.warnings.push(msg);
            }
        }
        if let Some(w) = credit_warn {
            let msg = format!("{}行目(貸方): {}", row_idx, w);
            if !report.warnings.contains(&msg) {
                report.warnings.push(msg);
            }
        }

        // Prevent identical debit and credit
        if debit_account == credit_account {
            report.errors.push(CsvRowError {
                row_index: report.total_data_rows,
                line_number: row_idx,
                message: format!(
                    "借方と貸方が同じ科目（{}）になっています",
                    debit_account.label_ja()
                ),
                raw_data: raw_line,
            });
            continue;
        }

        // 4. Description (join col 14, 15, 16 if non-empty)
        let desc1 = record.get(14).map(|s| s.trim()).unwrap_or("");
        let desc2 = record.get(15).map(|s| s.trim()).unwrap_or("");
        let desc3 = record.get(16).map(|s| s.trim()).unwrap_or("");

        let joined_desc = [desc1, desc2, desc3]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let final_desc = if joined_desc.is_empty() {
            format!("{}/{}", debit_account.label_ja(), credit_account.label_ja())
        } else {
            joined_desc
        };

        // Create JournalEntry
        let entry = JournalEntry::new(date, debit_account, credit_account, amount, final_desc);
        report.entries.push(entry);
    }

    report
}

/// Convenience entrypoint taking raw byte slice (from file upload or file read).
pub fn parse_legacy_csv(raw_bytes: &[u8], fiscal_year: i32) -> CsvImportReport {
    let decoded = decode_csv_bytes(raw_bytes);
    parse_legacy_csv_str(&decoded, fiscal_year)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sample_user_csv() {
        let sample = "\
日付,№,借　　　　方,,,貸　　　　方,,,,,集計科目コード１,集計科目コード２,集計除外摘要文１,集計除外摘要文２,摘　　要,摘要２,摘要３\n\
日付,№,科目ｺｰﾄﾞ,勘定科目,金　　額,科目ｺｰﾄﾞ,勘定科目,金　　額,月,重複チェック,1,3,家事消費等,雑収入,摘要,摘要２,摘要３\n\
1月5日,1,104,その他の預金,\"165,000.00\",1,売上（収入）,\"165,000.00\",1,FALSE,165000,0,0,0,賃料:(2026年1月分),,\n";

        let report = parse_legacy_csv_str(sample, 2026);
        assert_eq!(report.errors.len(), 0);
        assert_eq!(report.entries.len(), 1);

        let e = &report.entries[0];
        assert_eq!(e.date, NaiveDate::from_ymd_opt(2026, 1, 5).unwrap());
        assert_eq!(e.debit, AccountCode::Bank);
        assert_eq!(e.credit, AccountCode::Sales);
        assert_eq!(e.amount, 165_000);
        assert_eq!(e.description, "賃料:(2026年1月分)");
    }

    #[test]
    fn test_joined_descriptions_and_fallbacks() {
        let sample = "\
日付,№,借　　　　方,,,貸　　　　方,,,,,集計科目コード１,集計科目コード２,集計除外摘要文１,集計除外摘要文２,摘　　要,摘要２,摘要３\n\
日付,№,科目ｺｰﾄﾞ,勘定科目,金　　額,科目ｺｰﾄﾞ,勘定科目,金　　額,月,重複チェック,1,3,家事消費等,雑収入,摘要,摘要２,摘要３\n\
2月10日,2,510,消耗品費,\"3,500\",101,現金,\"3,500\",2,FALSE,0,0,0,0,ノートパソコンアクセサリ,Amazon購入,領収書あり\n\
3/15,3,999,未定義科目,\"12,000\",103,普通預金,\"12,000\",3,FALSE,0,0,0,0,テスト費用,,\n";

        let report = parse_legacy_csv_str(sample, 2026);
        assert_eq!(report.entries.len(), 2);

        // Entry 1 check
        let e1 = &report.entries[0];
        assert_eq!(e1.date, NaiveDate::from_ymd_opt(2026, 2, 10).unwrap());
        assert_eq!(e1.debit, AccountCode::Supplies);
        assert_eq!(e1.credit, AccountCode::Cash);
        assert_eq!(e1.amount, 3500);
        assert_eq!(
            e1.description,
            "ノートパソコンアクセサリ Amazon購入 領収書あり"
        );

        // Entry 2 check (fallback)
        let e2 = &report.entries[1];
        assert_eq!(e2.date, NaiveDate::from_ymd_opt(2026, 3, 15).unwrap());
        assert_eq!(e2.debit, AccountCode::Miscellaneous);
        assert_eq!(e2.credit, AccountCode::Bank);
        assert_eq!(e2.amount, 12000);
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn test_shift_jis_decoding() {
        // "その他の預金" encoded in Shift_JIS
        let sjis_bytes = [
            0x82, 0xbb, 0x82, 0xcc, 0x91, 0xbc, 0x82, 0xcc, 0x97, 0x61, 0x8b, 0xe0,
        ];
        let decoded = decode_csv_bytes(&sjis_bytes);
        assert_eq!(decoded, "その他の預金");
    }
}
