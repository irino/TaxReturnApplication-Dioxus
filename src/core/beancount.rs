//! Beancount (Plain Text Accounting) Import & Export Engine.
//!
//! Provides bidirectional interoperability between Japanese Blue Return double-entry ledgers
//! and the standardized Beancount text format (`.beancount`), supporting Git line-diff tracking
//! and visualization via Fava.

use crate::core::journal::{AccountCode, AccountNature, JournalEntry, LedgerDocument};
use chrono::NaiveDate;
use std::collections::BTreeSet;

/// 勘定科目および補助科目から Beancount 標準のアカウント名（Assets/Liabilities/Equity/Income/Expenses）を生成
pub fn account_to_beancount_name(code: &AccountCode, sub: Option<&str>) -> String {
    let sanitize = |s: &str| -> String {
        s.replace([':', ' ', '\t', '　', '/', '-'], "_")
    };

    let base = match code {
        AccountCode::Cash => "Assets:Cash".to_string(),
        AccountCode::Bank => {
            if let Some(s) = sub {
                format!("Assets:Bank:{}", sanitize(s))
            } else {
                "Assets:Bank:General".to_string()
            }
        }
        AccountCode::AccountsReceivable => {
            if let Some(s) = sub {
                format!("Assets:Receivables:{}", sanitize(s))
            } else {
                "Assets:Receivables".to_string()
            }
        }
        AccountCode::ToolsAndEquipment => "Assets:Equipment".to_string(),
        AccountCode::Vehicles => "Assets:Vehicles".to_string(),
        AccountCode::Buildings => "Assets:Buildings".to_string(),
        AccountCode::Software => "Assets:Software".to_string(),
        AccountCode::AccountsPayable => {
            if let Some(s) = sub {
                format!("Liabilities:Payables:{}", sanitize(s))
            } else {
                "Liabilities:Payables".to_string()
            }
        }
        AccountCode::Capital => "Equity:Capital".to_string(),
        AccountCode::Drawings => "Equity:Drawings".to_string(),
        AccountCode::OwnerContribution => "Liabilities:OwnerContribution".to_string(),
        AccountCode::Sales => "Income:Sales".to_string(),
        AccountCode::CostOfSales => "Expenses:CostOfSales".to_string(),
        AccountCode::Travel => "Expenses:Travel".to_string(),
        AccountCode::Communication => "Expenses:Communication".to_string(),
        AccountCode::Supplies => "Expenses:Supplies".to_string(),
        AccountCode::Rent => "Expenses:Rent".to_string(),
        AccountCode::Utilities => "Expenses:Utilities".to_string(),
        AccountCode::Advertising => "Expenses:Advertising".to_string(),
        AccountCode::Entertainment => "Expenses:Entertainment".to_string(),
        AccountCode::Insurance => "Expenses:Insurance".to_string(),
        AccountCode::Repair => "Expenses:Repair".to_string(),
        AccountCode::Depreciation => "Expenses:Depreciation".to_string(),
        AccountCode::Welfare => "Expenses:Welfare".to_string(),
        AccountCode::Salaries => "Expenses:Salaries".to_string(),
        AccountCode::Subcontracting => "Expenses:Subcontracting".to_string(),
        AccountCode::Interest => "Expenses:Interest".to_string(),
        AccountCode::TaxesAndDues => "Expenses:TaxesAndDues".to_string(),
        AccountCode::PackingShipping => "Expenses:PackingShipping".to_string(),
        AccountCode::Miscellaneous => "Expenses:Miscellaneous".to_string(),
        AccountCode::Custom(name) => {
            let cat = match code.nature() {
                AccountNature::Asset => "Assets:Custom",
                AccountNature::Liability => "Liabilities:Custom",
                AccountNature::Equity => "Equity:Custom",
                AccountNature::Income => "Income:Custom",
                AccountNature::Expense => "Expenses:Custom",
            };
            format!("{cat}:{}", sanitize(name))
        }
    };

    base
}

/// Beancount のアカウント名から勘定科目と補助科目を推定・復元
pub fn beancount_name_to_account(beancount_name: &str) -> (AccountCode, Option<String>) {
    let trimmed = beancount_name.trim();

    if trimmed == "Assets:Cash" {
        return (AccountCode::Cash, None);
    }
    if let Some(sub) = trimmed.strip_prefix("Assets:Bank:") {
        return (AccountCode::Bank, Some(sub.to_string()));
    }
    if trimmed == "Assets:Bank" || trimmed == "Assets:Bank:General" {
        return (AccountCode::Bank, None);
    }
    if let Some(sub) = trimmed.strip_prefix("Assets:Receivables:") {
        return (AccountCode::AccountsReceivable, Some(sub.to_string()));
    }
    if trimmed == "Assets:Receivables" {
        return (AccountCode::AccountsReceivable, None);
    }
    if trimmed == "Assets:Equipment" {
        return (AccountCode::ToolsAndEquipment, None);
    }
    if trimmed == "Assets:Vehicles" {
        return (AccountCode::Vehicles, None);
    }
    if trimmed == "Assets:Buildings" {
        return (AccountCode::Buildings, None);
    }
    if trimmed == "Assets:Software" {
        return (AccountCode::Software, None);
    }
    if let Some(sub) = trimmed.strip_prefix("Liabilities:Payables:") {
        return (AccountCode::AccountsPayable, Some(sub.to_string()));
    }
    if trimmed == "Liabilities:Payables" {
        return (AccountCode::AccountsPayable, None);
    }
    if trimmed == "Equity:Capital" || trimmed == "Equity:OpeningBalances" {
        return (AccountCode::Capital, None);
    }
    if trimmed == "Equity:Drawings" {
        return (AccountCode::Drawings, None);
    }
    if trimmed == "Liabilities:OwnerContribution" {
        return (AccountCode::OwnerContribution, None);
    }
    if trimmed.starts_with("Income:Sales") {
        return (AccountCode::Sales, None);
    }
    if trimmed == "Expenses:CostOfSales" {
        return (AccountCode::CostOfSales, None);
    }
    if trimmed == "Expenses:Travel" {
        return (AccountCode::Travel, None);
    }
    if trimmed == "Expenses:Communication" {
        return (AccountCode::Communication, None);
    }
    if trimmed == "Expenses:Supplies" {
        return (AccountCode::Supplies, None);
    }
    if trimmed == "Expenses:Rent" {
        return (AccountCode::Rent, None);
    }
    if trimmed == "Expenses:Utilities" {
        return (AccountCode::Utilities, None);
    }
    if trimmed == "Expenses:Advertising" {
        return (AccountCode::Advertising, None);
    }
    if trimmed == "Expenses:Entertainment" {
        return (AccountCode::Entertainment, None);
    }
    if trimmed == "Expenses:Insurance" {
        return (AccountCode::Insurance, None);
    }
    if trimmed == "Expenses:Repair" {
        return (AccountCode::Repair, None);
    }
    if trimmed == "Expenses:Depreciation" {
        return (AccountCode::Depreciation, None);
    }
    if trimmed == "Expenses:Welfare" {
        return (AccountCode::Welfare, None);
    }
    if trimmed == "Expenses:Salaries" {
        return (AccountCode::Salaries, None);
    }
    if trimmed == "Expenses:Subcontracting" {
        return (AccountCode::Subcontracting, None);
    }
    if trimmed == "Expenses:Interest" {
        return (AccountCode::Interest, None);
    }
    if trimmed == "Expenses:TaxesAndDues" {
        return (AccountCode::TaxesAndDues, None);
    }
    if trimmed == "Expenses:PackingShipping" {
        return (AccountCode::PackingShipping, None);
    }
    if trimmed == "Expenses:Miscellaneous" {
        return (AccountCode::Miscellaneous, None);
    }

    // カスタム科目推定
    let parts: Vec<&str> = trimmed.split(':').collect();
    let last = parts.last().copied().unwrap_or(trimmed);
    (AccountCode::Custom(last.to_string()), None)
}

/// LedgerDocument を Beancount 形式のプレーンテキストとして出力
pub fn export_to_beancount(doc: &LedgerDocument) -> String {
    let mut out = String::new();

    // ヘッダーメタデータ
    out.push_str(";; -*- mode: beancount -*-\n");
    out.push_str(&format!(
        ";; Generated by TaxReturnApplication-Dioxus for Fiscal Year {}\n",
        doc.fiscal_year
    ));
    if !doc.taxpayer.name.is_empty() {
        out.push_str(&format!(";; Taxpayer: {}\n", doc.taxpayer.name));
    }
    out.push_str("option \"title\" \"確定申告 会計帳簿\"\n");
    out.push_str("option \"operating_currency\" \"JPY\"\n\n");

    // 全登場アカウントの open ディレクティブを収集して出力
    let mut accounts = BTreeSet::new();
    for e in &doc.entries {
        accounts.insert(account_to_beancount_name(&e.debit, e.debit_sub.as_deref()));
        accounts.insert(account_to_beancount_name(&e.credit, e.credit_sub.as_deref()));
    }

    let open_date = format!("{}-01-01", doc.fiscal_year);
    for acc in &accounts {
        out.push_str(&format!("{open_date} open {acc} JPY\n"));
    }
    out.push('\n');

    // 日付昇順で仕訳を出力
    let mut sorted_entries = doc.entries.clone();
    sorted_entries.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.created_at.cmp(&b.created_at)));

    for e in &sorted_entries {
        let debit_acc = account_to_beancount_name(&e.debit, e.debit_sub.as_deref());
        let credit_acc = account_to_beancount_name(&e.credit, e.credit_sub.as_deref());

        let date_str = e.date.format("%Y-%m-%d");
        let escaped_desc = e.description.replace('"', "\\\"");

        out.push_str(&format!("{date_str} * \"{escaped_desc}\"\n"));
        // Beancount convention: 借方はプラス、貸方はマイナス
        out.push_str(&format!("  {:<36} {:>10} JPY\n", debit_acc, e.amount));
        out.push_str(&format!("  {:<36} {:>10} JPY\n", credit_acc, -e.amount));
        out.push('\n');
    }

    out
}

/// Beancount テキストから仕訳をパース・インポート
pub fn import_from_beancount(content: &str, target_year: i32) -> Result<Vec<JournalEntry>, String> {
    let mut entries = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // コメント・空行・ディレクティブ行をスキップ
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') || line.starts_with("option ") {
            i += 1;
            continue;
        }

        // 取引行: "YYYY-MM-DD * \"description\""
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            i += 1;
            continue;
        }

        if let Ok(date) = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d") {
            if date.format("%Y").to_string().parse::<i32>().unwrap_or(0) != target_year {
                // 対象年度外の取引はスキップ
                i += 1;
                continue;
            }

            // 摘要の抽出
            let mut description = String::new();
            if let Some(first_quote) = line.find('"') {
                if let Some(last_quote) = line.rfind('"') {
                    if last_quote > first_quote {
                        description = line[first_quote + 1..last_quote].to_string();
                    }
                }
            }
            if description.is_empty() {
                description = "Beancountインポート仕訳".to_string();
            }

            // 配下の Posting 行を走査
            i += 1;
            let mut postings: Vec<(AccountCode, Option<String>, i64)> = Vec::new();

            while i < lines.len() {
                let p_line = lines[i];
                if p_line.trim().is_empty() || !p_line.starts_with(' ') && !p_line.starts_with('\t') {
                    break;
                }

                let p_parts: Vec<&str> = p_line.split_whitespace().collect();
                if p_parts.len() >= 2 {
                    let acc_name = p_parts[0];
                    let (code, sub) = beancount_name_to_account(acc_name);

                    // 金額部分を取得
                    let amt_str = p_parts[1].replace(',', "");
                    if let Ok(signed_amt) = amt_str.parse::<i64>() {
                        postings.push((code, sub, signed_amt));
                    }
                }
                i += 1;
            }

            // 借方（プラス）と貸方（マイナス）を対応付けて JournalEntry 化
            let debits: Vec<&(AccountCode, Option<String>, i64)> =
                postings.iter().filter(|p| p.2 > 0).collect();
            let credits: Vec<&(AccountCode, Option<String>, i64)> =
                postings.iter().filter(|p| p.2 < 0).collect();

            if debits.len() == 1 && credits.len() == 1 {
                let d = debits[0];
                let c = credits[0];
                entries.push(JournalEntry::new_with_sub(
                    date,
                    d.0.clone(),
                    d.1.clone(),
                    c.0.clone(),
                    c.1.clone(),
                    d.2,
                    description,
                ));
            } else if !debits.is_empty() && !credits.is_empty() {
                // 1:N or N:1 の複合取引を展開
                for d in &debits {
                    for c in &credits {
                        let amt = d.2.min(-c.2);
                        if amt > 0 {
                            entries.push(JournalEntry::new_with_sub(
                                date,
                                d.0.clone(),
                                d.1.clone(),
                                c.0.clone(),
                                c.1.clone(),
                                amt,
                                description.clone(),
                            ));
                        }
                    }
                }
            }

            continue;
        }

        i += 1;
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beancount_export_and_import_roundtrip() {
        let mut doc = LedgerDocument::new(2025);
        doc.entries.push(JournalEntry::new_with_sub(
            NaiveDate::from_ymd_opt(2025, 4, 15).unwrap(),
            AccountCode::Bank,
            Some("三井住友".into()),
            AccountCode::Sales,
            None,
            150_000,
            "4月分コンサル報酬",
        ));
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 4, 20).unwrap(),
            AccountCode::Rent,
            AccountCode::Bank,
            80_000,
            "事務所家賃",
        ));

        let beancount_text = export_to_beancount(&doc);
        assert!(beancount_text.contains("option \"operating_currency\" \"JPY\""));
        assert!(beancount_text.contains("open Assets:Bank:三井住友 JPY"));
        assert!(beancount_text.contains("open Income:Sales JPY"));
        assert!(beancount_text.contains("150000 JPY"));
        assert!(beancount_text.contains("-150000 JPY"));

        // 再インポート
        let imported = import_from_beancount(&beancount_text, 2025).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].amount, 150_000);
        assert_eq!(imported[0].debit, AccountCode::Bank);
        assert_eq!(imported[0].debit_sub.as_deref(), Some("三井住友"));
        assert_eq!(imported[0].credit, AccountCode::Sales);
        assert_eq!(imported[1].amount, 80_000);
        assert_eq!(imported[1].debit, AccountCode::Rent);
    }
}
