//! Journal structures, chart of accounts, and bookkeeping validation.

use chrono::{DateTime, Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Schema version written into persisted ledger documents.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// High-level account nature for balance / P&L classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountNature {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

/// Minimal chart of accounts for sole-proprietor bookkeeping.
/// Stable serde names so saved JSON survives renames of Rust variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountCode {
    // Assets
    Cash,
    Bank,
    AccountsReceivable,
    // Liabilities
    AccountsPayable,
    // Equity / owner
    Capital,
    Drawings,
    OwnerContribution,
    // Income
    Sales,
    // Expenses
    CostOfSales,
    Travel,
    Communication,
    Supplies,
    Rent,
    Utilities,
    Miscellaneous,
}

impl AccountCode {
    /// All accounts in stable display order.
    pub fn all() -> &'static [AccountCode] {
        use AccountCode::*;
        &[
            Cash,
            Bank,
            AccountsReceivable,
            AccountsPayable,
            Capital,
            Drawings,
            OwnerContribution,
            Sales,
            CostOfSales,
            Travel,
            Communication,
            Supplies,
            Rent,
            Utilities,
            Miscellaneous,
        ]
    }

    pub fn label_ja(self) -> &'static str {
        match self {
            Self::Cash => "現金",
            Self::Bank => "普通預金",
            Self::AccountsReceivable => "売掛金",
            Self::AccountsPayable => "買掛金",
            Self::Capital => "元入金",
            Self::Drawings => "事業主貸",
            Self::OwnerContribution => "事業主借",
            Self::Sales => "売上高",
            Self::CostOfSales => "仕入高",
            Self::Travel => "旅費交通費",
            Self::Communication => "通信費",
            Self::Supplies => "消耗品費",
            Self::Rent => "地代家賃",
            Self::Utilities => "水道光熱費",
            Self::Miscellaneous => "雑費",
        }
    }

    pub fn nature(self) -> AccountNature {
        match self {
            Self::Cash | Self::Bank | Self::AccountsReceivable | Self::Drawings => {
                AccountNature::Asset
            }
            Self::AccountsPayable | Self::OwnerContribution => AccountNature::Liability,
            Self::Capital => AccountNature::Equity,
            Self::Sales => AccountNature::Income,
            Self::CostOfSales
            | Self::Travel
            | Self::Communication
            | Self::Supplies
            | Self::Rent
            | Self::Utilities
            | Self::Miscellaneous => AccountNature::Expense,
        }
    }
}

/// Single double-entry style journal line (one debit, one credit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub date: NaiveDate,
    pub debit: AccountCode,
    pub credit: AccountCode,
    /// Amount in whole yen. Always positive when valid.
    pub amount: i64,
    pub description: String,
    pub created_at: DateTime<Local>,
}

impl JournalEntry {
    pub fn new(
        date: NaiveDate,
        debit: AccountCode,
        credit: AccountCode,
        amount: i64,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            date,
            debit,
            credit,
            amount,
            description: description.into(),
            created_at: Local::now(),
        }
    }
}

/// 納税者・事業所の基本情報（帳票 PDF 用）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TaxpayerInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub business_name: String,
    #[serde(default)]
    pub phone: String,
}

/// One fiscal-year ledger document (JSON file payload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerDocument {
    pub version: u32,
    pub fiscal_year: i32,
    pub entries: Vec<JournalEntry>,
    /// 納税者情報（旧ファイル互換のため default）
    #[serde(default)]
    pub taxpayer: TaxpayerInfo,
    /// 青色申告特別控除の選択（旧ファイルは 10万円扱い）
    #[serde(default)]
    pub special_deduction: crate::core::tax_calc::BlueSpecialDeductionKind,
}

impl LedgerDocument {
    pub fn new(fiscal_year: i32) -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            fiscal_year,
            entries: Vec::new(),
            taxpayer: TaxpayerInfo::default(),
            special_deduction: crate::core::tax_calc::BlueSpecialDeductionKind::default(),
        }
    }

    pub fn sorted_entries(&self) -> Vec<&JournalEntry> {
        let mut items: Vec<&JournalEntry> = self.entries.iter().collect();
        items.sort_by(|a, b| {
            b.date
                .cmp(&a.date)
                .then_with(|| b.created_at.cmp(&a.created_at))
        });
        items
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JournalError {
    #[error("金額は1円以上で入力してください")]
    NonPositiveAmount,
    #[error("借方と貸方に同じ科目は指定できません")]
    SameDebitCredit,
    #[error("摘要が空です")]
    EmptyDescription,
    #[error("日付が会計年度（{year}年）の範囲外です")]
    DateOutsideFiscalYear { year: i32 },
}

/// Validate a journal entry for a given fiscal year (calendar year).
pub fn validate_entry(entry: &JournalEntry, fiscal_year: i32) -> Result<(), JournalError> {
    if entry.amount <= 0 {
        return Err(JournalError::NonPositiveAmount);
    }
    if entry.debit == entry.credit {
        return Err(JournalError::SameDebitCredit);
    }
    if entry.description.trim().is_empty() {
        return Err(JournalError::EmptyDescription);
    }
    if entry.date.year() != fiscal_year {
        return Err(JournalError::DateOutsideFiscalYear { year: fiscal_year });
    }
    Ok(())
}

/// Format yen with thousands separators (e.g. `1,234`).
pub fn format_yen(amount: i64) -> String {
    let negative = amount < 0;
    let digits = amount.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let formatted: String = out.chars().rev().collect();
    if negative {
        format!("-{formatted}")
    } else {
        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn sample_entry(amount: i64, debit: AccountCode, credit: AccountCode) -> JournalEntry {
        JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
            debit,
            credit,
            amount,
            "テスト仕訳",
        )
    }

    #[test]
    fn validate_accepts_normal_entry() {
        let entry = sample_entry(10_000, AccountCode::Cash, AccountCode::Sales);
        assert!(validate_entry(&entry, 2025).is_ok());
    }

    #[test]
    fn reject_zero_and_negative_amount() {
        let zero = sample_entry(0, AccountCode::Cash, AccountCode::Sales);
        let neg = sample_entry(-1, AccountCode::Cash, AccountCode::Sales);
        assert_eq!(
            validate_entry(&zero, 2025),
            Err(JournalError::NonPositiveAmount)
        );
        assert_eq!(
            validate_entry(&neg, 2025),
            Err(JournalError::NonPositiveAmount)
        );
    }

    #[test]
    fn reject_same_debit_credit() {
        let entry = sample_entry(100, AccountCode::Cash, AccountCode::Cash);
        assert_eq!(
            validate_entry(&entry, 2025),
            Err(JournalError::SameDebitCredit)
        );
    }

    #[test]
    fn reject_empty_description() {
        let mut entry = sample_entry(100, AccountCode::Bank, AccountCode::Sales);
        entry.description = "   ".into();
        assert_eq!(
            validate_entry(&entry, 2025),
            Err(JournalError::EmptyDescription)
        );
    }

    #[test]
    fn reject_date_outside_fiscal_year() {
        let entry = sample_entry(100, AccountCode::Bank, AccountCode::Sales);
        assert_eq!(
            validate_entry(&entry, 2024),
            Err(JournalError::DateOutsideFiscalYear { year: 2024 })
        );
    }

    #[test]
    fn account_labels_and_nature() {
        assert_eq!(AccountCode::Sales.label_ja(), "売上高");
        assert_eq!(AccountCode::Sales.nature(), AccountNature::Income);
        assert_eq!(AccountCode::Travel.nature(), AccountNature::Expense);
        assert_eq!(AccountCode::Cash.nature(), AccountNature::Asset);
    }

    #[test]
    fn format_yen_thousands() {
        assert_eq!(format_yen(0), "0");
        assert_eq!(format_yen(1234), "1,234");
        assert_eq!(format_yen(-1_000_000), "-1,000,000");
    }

    #[test]
    fn ledger_document_serde_roundtrip() {
        let mut doc = LedgerDocument::new(2025);
        doc.entries.push(sample_entry(
            50_000,
            AccountCode::Bank,
            AccountCode::Sales,
        ));
        let json = serde_json::to_string_pretty(&doc).unwrap();
        let back: LedgerDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, CURRENT_SCHEMA_VERSION);
        assert_eq!(back.fiscal_year, 2025);
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].amount, 50_000);
        assert_eq!(back.entries[0].debit, AccountCode::Bank);
    }

    #[test]
    fn sorted_entries_newest_first() {
        let mut doc = LedgerDocument::new(2025);
        let older = JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            AccountCode::Cash,
            AccountCode::Sales,
            1,
            "old",
        );
        let newer = JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            AccountCode::Cash,
            AccountCode::Sales,
            2,
            "new",
        );
        doc.entries.push(older);
        doc.entries.push(newer);
        let sorted = doc.sorted_entries();
        assert_eq!(sorted[0].description, "new");
        assert_eq!(sorted[1].description, "old");
    }
}
