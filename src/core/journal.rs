//! Journal structures, chart of accounts, sub-accounts, and bookkeeping validation.

use crate::core::deductions::TaxDeductions;
use crate::core::depreciation::FixedAsset;
use chrono::{DateTime, Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Schema version written into persisted ledger documents.
pub const CURRENT_SCHEMA_VERSION: u32 = 4;

/// High-level account nature for balance / P&L classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountNature {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

impl AccountNature {
    pub fn label_ja(self) -> &'static str {
        match self {
            Self::Asset => "資産",
            Self::Liability => "負債",
            Self::Equity => "純資産",
            Self::Income => "収益",
            Self::Expense => "費用",
        }
    }
}

/// Chart of accounts for sole-proprietor bookkeeping.
/// Includes standard Japanese Blue Return accounts and supports custom user accounts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountCode {
    // Assets (資産)
    Cash,
    Bank,
    AccountsReceivable,
    ToolsAndEquipment, // 工具器具備品
    Vehicles,          // 車両運搬具
    Buildings,         // 建物
    Software,          // ソフトウェア

    // Liabilities (負債)
    AccountsPayable,

    // Equity / Owner (純資産)
    Capital,
    Drawings,
    OwnerContribution,

    // Income (収益)
    Sales,

    // Expenses (費用 - 基本15科目)
    CostOfSales,
    Travel,
    Communication,
    Supplies,
    Rent,
    Utilities,
    Miscellaneous,

    // Expenses (費用 - 追加標準青色申告科目)
    Advertising,      // 広告宣伝費
    Entertainment,    // 接待交際費
    Insurance,        // 損害保険料
    Repair,           // 修繕費
    Depreciation,     // 減価償却費
    Welfare,          // 福利厚生費
    Salaries,         // 給料賃金
    Subcontracting,   // 外注工賃
    Interest,         // 利子割引料
    TaxesAndDues,     // 租税公課
    PackingShipping,  // 荷造運賃

    // ユーザー定義カスタム科目
    #[serde(untagged)]
    Custom(String),
}

const STANDARD_ACCOUNTS: &[AccountCode] = &[
    AccountCode::Cash,
    AccountCode::Bank,
    AccountCode::AccountsReceivable,
    AccountCode::ToolsAndEquipment,
    AccountCode::Vehicles,
    AccountCode::Buildings,
    AccountCode::Software,
    AccountCode::AccountsPayable,
    AccountCode::Capital,
    AccountCode::Drawings,
    AccountCode::OwnerContribution,
    AccountCode::Sales,
    AccountCode::CostOfSales,
    AccountCode::Travel,
    AccountCode::Communication,
    AccountCode::Supplies,
    AccountCode::Rent,
    AccountCode::Utilities,
    AccountCode::Advertising,
    AccountCode::Entertainment,
    AccountCode::Insurance,
    AccountCode::Repair,
    AccountCode::Depreciation,
    AccountCode::Welfare,
    AccountCode::Salaries,
    AccountCode::Subcontracting,
    AccountCode::Interest,
    AccountCode::TaxesAndDues,
    AccountCode::PackingShipping,
    AccountCode::Miscellaneous,
];

impl AccountCode {
    /// All standard accounts in canonical display order.
    pub fn all() -> &'static [AccountCode] {
        STANDARD_ACCOUNTS
    }

    pub fn label_ja(&self) -> &str {
        match self {
            Self::Cash => "現金",
            Self::Bank => "普通預金",
            Self::AccountsReceivable => "売掛金",
            Self::ToolsAndEquipment => "工具器具備品",
            Self::Vehicles => "車両運搬具",
            Self::Buildings => "建物",
            Self::Software => "ソフトウェア",
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
            Self::Advertising => "広告宣伝費",
            Self::Entertainment => "接待交際費",
            Self::Insurance => "損害保険料",
            Self::Repair => "修繕費",
            Self::Depreciation => "減価償却費",
            Self::Welfare => "福利厚生費",
            Self::Salaries => "給料賃金",
            Self::Subcontracting => "外注工賃",
            Self::Interest => "利子割引料",
            Self::TaxesAndDues => "租税公課",
            Self::PackingShipping => "荷造運賃",
            Self::Miscellaneous => "雑費",
            Self::Custom(name) => name.as_str(),
        }
    }

    pub fn nature(&self) -> AccountNature {
        match self {
            Self::Cash
            | Self::Bank
            | Self::AccountsReceivable
            | Self::Drawings
            | Self::ToolsAndEquipment
            | Self::Vehicles
            | Self::Buildings
            | Self::Software => AccountNature::Asset,
            Self::AccountsPayable | Self::OwnerContribution => AccountNature::Liability,
            Self::Capital => AccountNature::Equity,
            Self::Sales => AccountNature::Income,
            Self::CostOfSales
            | Self::Travel
            | Self::Communication
            | Self::Supplies
            | Self::Rent
            | Self::Utilities
            | Self::Advertising
            | Self::Entertainment
            | Self::Insurance
            | Self::Repair
            | Self::Depreciation
            | Self::Welfare
            | Self::Salaries
            | Self::Subcontracting
            | Self::Interest
            | Self::TaxesAndDues
            | Self::PackingShipping
            | Self::Miscellaneous => AccountNature::Expense,
            Self::Custom(_) => AccountNature::Expense,
        }
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }
}

/// 勘定科目のマスタ定義（名前・性質・補助科目リスト）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountDefinition {
    pub code: AccountCode,
    pub name: String,
    pub nature: AccountNature,
    pub is_custom: bool,
    #[serde(default)]
    pub sub_accounts: Vec<String>,
}

impl AccountDefinition {
    pub fn new_standard(code: AccountCode, sub_accounts: Vec<String>) -> Self {
        Self {
            name: code.label_ja().to_string(),
            nature: code.nature(),
            code,
            is_custom: false,
            sub_accounts,
        }
    }

    pub fn new_custom(
        name: impl Into<String>,
        nature: AccountNature,
        sub_accounts: Vec<String>,
    ) -> Self {
        let name_str = name.into();
        Self {
            code: AccountCode::Custom(name_str.clone()),
            name: name_str,
            nature,
            is_custom: true,
            sub_accounts,
        }
    }

    /// Default registry with canonical accounts and useful sub-account presets.
    pub fn default_registry() -> Vec<Self> {
        let mut list = Vec::new();
        for code in AccountCode::all() {
            let subs = match code {
                AccountCode::Bank => vec![
                    "三井住友銀行".into(),
                    "三菱UFJ銀行".into(),
                    "みずほ銀行".into(),
                    "ゆうちょ銀行".into(),
                    "PayPay銀行".into(),
                    "楽天銀行".into(),
                ],
                AccountCode::AccountsReceivable => vec!["主要取引先A".into(), "主要取引先B".into()],
                AccountCode::AccountsPayable => vec!["仕入先X".into(), "クレジットカード".into()],
                _ => Vec::new(),
            };
            list.push(Self::new_standard(code.clone(), subs));
        }
        list
    }
}

/// Single double-entry style journal line (one debit, one credit) with optional sub-accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub date: NaiveDate,
    pub debit: AccountCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debit_sub: Option<String>,
    pub credit: AccountCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_sub: Option<String>,
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
        Self::new_with_sub(date, debit, None, credit, None, amount, description)
    }

    pub fn new_with_sub(
        date: NaiveDate,
        debit: AccountCode,
        debit_sub: Option<String>,
        credit: AccountCode,
        credit_sub: Option<String>,
        amount: i64,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            date,
            debit,
            debit_sub: debit_sub.and_then(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }),
            credit,
            credit_sub: credit_sub.and_then(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }),
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

/// 家事按分ルール（科目別の事業利用割合）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationRule {
    pub account: AccountCode,
    /// 事業割合 (0〜100%)
    pub business_ratio_percent: u8,
    /// 特定の補助科目に限定する場合（任意）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_account: Option<String>,
}

/// One fiscal-year ledger document (JSON file payload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerDocument {
    pub version: u32,
    pub fiscal_year: i32,
    pub entries: Vec<JournalEntry>,
    /// 納税者情報
    #[serde(default)]
    pub taxpayer: TaxpayerInfo,
    /// 青色申告特別控除の選択
    #[serde(default)]
    pub special_deduction: crate::core::tax_calc::BlueSpecialDeductionKind,
    /// 勘定科目・補助科目マスタ
    #[serde(default = "AccountDefinition::default_registry")]
    pub accounts: Vec<AccountDefinition>,
    /// 家事按分ルール
    #[serde(default)]
    pub allocation_rules: Vec<AllocationRule>,
    /// 固定資産台帳
    #[serde(default)]
    pub fixed_assets: Vec<FixedAsset>,
    /// 各種所得控除・税額控除設定
    #[serde(default)]
    pub deductions: TaxDeductions,
}

impl LedgerDocument {
    pub fn new(fiscal_year: i32) -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            fiscal_year,
            entries: Vec::new(),
            taxpayer: TaxpayerInfo::default(),
            special_deduction: crate::core::tax_calc::BlueSpecialDeductionKind::default(),
            accounts: AccountDefinition::default_registry(),
            allocation_rules: Vec::new(),
            fixed_assets: Vec::new(),
            deductions: TaxDeductions::default(),
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

    /// 科目定義を検索
    pub fn find_account(&self, code: &AccountCode) -> Option<&AccountDefinition> {
        self.accounts.iter().find(|a| &a.code == code)
    }

    /// カスタム科目を追加
    pub fn add_custom_account(
        &mut self,
        name: &str,
        nature: AccountNature,
    ) -> Result<AccountCode, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("科目名を入力してください".into());
        }
        if self.accounts.iter().any(|a| a.name == trimmed) {
            return Err(format!("科目「{trimmed}」は既に存在します"));
        }
        let def = AccountDefinition::new_custom(trimmed, nature, Vec::new());
        let code = def.code.clone();
        self.accounts.push(def);
        Ok(code)
    }

    /// カスタム科目を削除
    pub fn remove_custom_account(&mut self, code: &AccountCode) -> Result<(), String> {
        if !code.is_custom() {
            return Err("標準科目は削除できません".into());
        }
        // 仕訳で使用されているか確認
        if self
            .entries
            .iter()
            .any(|e| &e.debit == code || &e.credit == code)
        {
            return Err("この科目を使用した仕訳が存在するため削除できません".into());
        }
        self.accounts.retain(|a| &a.code != code);
        // 按分ルールからも削除
        self.allocation_rules.retain(|r| &r.account != code);
        Ok(())
    }

    /// 補助科目を追加
    pub fn add_sub_account(
        &mut self,
        code: &AccountCode,
        sub_name: &str,
    ) -> Result<(), String> {
        let trimmed = sub_name.trim();
        if trimmed.is_empty() {
            return Err("補助科目名を入力してください".into());
        }
        let def = self
            .accounts
            .iter_mut()
            .find(|a| &a.code == code)
            .ok_or_else(|| "該当の科目がマスタに見つかりません".to_string())?;

        if def.sub_accounts.iter().any(|s| s == trimmed) {
            return Err(format!("補助科目「{trimmed}」は既に登録されています"));
        }
        def.sub_accounts.push(trimmed.to_string());
        Ok(())
    }

    /// 補助科目を削除
    pub fn remove_sub_account(
        &mut self,
        code: &AccountCode,
        sub_name: &str,
    ) -> Result<(), String> {
        let def = self
            .accounts
            .iter_mut()
            .find(|a| &a.code == code)
            .ok_or_else(|| "該当の科目がマスタに見つかりません".to_string())?;

        def.sub_accounts.retain(|s| s != sub_name);
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JournalError {
    #[error("金額は1円以上で入力してください")]
    NonPositiveAmount,
    #[error("借方と貸方に同じ科目は指定できません（異なる補助科目間の振替を除く）")]
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
    // 同一科目かつ補助科目も同一（または両方なし）の場合はエラー
    if entry.debit == entry.credit {
        let is_different_sub = match (&entry.debit_sub, &entry.credit_sub) {
            (Some(d), Some(c)) => d != c,
            _ => false,
        };
        if !is_different_sub {
            return Err(JournalError::SameDebitCredit);
        }
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
    fn validate_accepts_same_account_with_different_sub_accounts() {
        let entry = JournalEntry::new_with_sub(
            NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
            AccountCode::Bank,
            Some("三井住友".into()),
            AccountCode::Bank,
            Some("みずほ".into()),
            50_000,
            "銀行間振替",
        );
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
        assert_eq!(AccountCode::Advertising.label_ja(), "広告宣伝費");
        assert_eq!(AccountCode::Custom("クラウド利用料".into()).label_ja(), "クラウド利用料");
        assert!(AccountCode::Custom("テスト".into()).is_custom());
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
        doc.entries.push(JournalEntry::new_with_sub(
            NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
            AccountCode::Bank,
            Some("三井住友".into()),
            AccountCode::Sales,
            None,
            50_000,
            "売上",
        ));
        doc.add_custom_account("支払手数料", AccountNature::Expense).unwrap();
        doc.fixed_assets.push(FixedAsset::new(
            "PC",
            NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
            200_000,
            4,
            crate::core::depreciation::DepreciationMethod::StraightLine,
            100,
            AccountCode::ToolsAndEquipment,
            "ノートPC",
        ));
        doc.deductions.social_insurance = 300_000;

        let json = serde_json::to_string_pretty(&doc).unwrap();
        let back: LedgerDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, CURRENT_SCHEMA_VERSION);
        assert_eq!(back.fiscal_year, 2025);
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].amount, 50_000);
        assert_eq!(back.entries[0].debit, AccountCode::Bank);
        assert_eq!(back.entries[0].debit_sub.as_deref(), Some("三井住友"));
        assert!(back.accounts.iter().any(|a| a.name == "支払手数料"));
        assert_eq!(back.fixed_assets.len(), 1);
        assert_eq!(back.fixed_assets[0].name, "PC");
        assert_eq!(back.deductions.social_insurance, 300_000);
    }

    #[test]
    fn backwards_compatibility_deserialization() {
        // v2 style JSON without accounts, allocation_rules, fixed_assets, deductions, or debit_sub
        let old_json = r#"{
            "version": 2,
            "fiscal_year": 2025,
            "entries": [
                {
                    "id": "abc-123",
                    "date": "2025-05-01",
                    "debit": "cash",
                    "credit": "sales",
                    "amount": 10000,
                    "description": "旧仕訳",
                    "created_at": "2025-05-01T12:00:00+09:00"
                }
            ]
        }"#;

        let doc: LedgerDocument = serde_json::from_str(old_json).unwrap();
        assert_eq!(doc.fiscal_year, 2025);
        assert_eq!(doc.entries.len(), 1);
        assert_eq!(doc.entries[0].debit, AccountCode::Cash);
        assert_eq!(doc.entries[0].debit_sub, None);
        assert!(!doc.accounts.is_empty()); // Default registry populated!
        assert!(doc.fixed_assets.is_empty());
        assert_eq!(doc.deductions, TaxDeductions::default());
    }

    #[test]
    fn custom_account_and_sub_account_management() {
        let mut doc = LedgerDocument::new(2025);
        let custom_code = doc.add_custom_account("外注工賃カスタム", AccountNature::Expense).unwrap();
        assert!(doc.find_account(&custom_code).is_some());

        // Sub-account
        doc.add_sub_account(&AccountCode::Bank, "新銀行").unwrap();
        let bank_def = doc.find_account(&AccountCode::Bank).unwrap();
        assert!(bank_def.sub_accounts.contains(&"新銀行".to_string()));

        doc.remove_sub_account(&AccountCode::Bank, "新銀行").unwrap();
        let bank_def = doc.find_account(&AccountCode::Bank).unwrap();
        assert!(!bank_def.sub_accounts.contains(&"新銀行".to_string()));

        // Cannot delete standard account
        assert!(doc.remove_custom_account(&AccountCode::Cash).is_err());
        // Can delete custom account
        assert!(doc.remove_custom_account(&custom_code).is_ok());
    }
}
