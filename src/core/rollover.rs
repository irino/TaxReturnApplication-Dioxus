//! Fiscal year balance rollover (年度繰越処理) engine.
//!
//! Pure accounting logic for rolling over ending balance sheet items (Assets, Liabilities)
//! and recalculating the beginning Capital (期首元入金) into the next fiscal year
//! according to Japanese Blue Return sole-proprietor bookkeeping standards.

use crate::core::journal::{
    AccountCode, AccountNature, JournalEntry, LedgerDocument,
};
use crate::core::tax_calc::aggregate_blue_return;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 繰越前の確認プレビュー情報。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloverPreview {
    pub from_year: i32,
    pub to_year: i32,
    /// 繰り越される資産科目残高 (科目名, 金額)
    pub asset_balances: Vec<(String, i64)>,
    pub total_assets: i64,
    /// 繰り越される負債科目残高 (科目名, 金額)
    pub liability_balances: Vec<(String, i64)>,
    pub total_liabilities: i64,
    /// 元入金の計算内訳
    pub prev_capital: i64,
    pub net_income: i64,
    pub owner_contribution: i64,
    pub drawings: i64,
    /// 翌期首元入金 = prev_capital + net_income + owner_contribution - drawings
    pub next_capital: i64,
}

const ROLLOVER_MEMO: &str = "前年度より繰越（期首残高）";

/// 該当仕訳が年度繰越による期首残高仕訳かどうかを判定。
pub fn is_rollover_entry(entry: &JournalEntry) -> bool {
    entry.description == ROLLOVER_MEMO
        && (entry.debit == AccountCode::Capital || entry.credit == AccountCode::Capital)
}

/// 前年度ドキュメントから繰越プレビューを計算。
pub fn calculate_rollover_preview(prev_doc: &LedgerDocument) -> RolloverPreview {
    let from_year = prev_doc.fiscal_year;
    let to_year = from_year + 1;

    // 青色申告損益集計を実行して、当期純利益（事業所得）および各科目残高を取得
    let blue = aggregate_blue_return(&prev_doc.entries, from_year, prev_doc.special_deduction);

    // 科目別の符号付き正味残高を集計（補助科目別ではなく科目別）
    let mut account_balances: BTreeMap<AccountCode, i64> = BTreeMap::new();
    for entry in &prev_doc.entries {
        *account_balances.entry(entry.debit.clone()).or_default() += entry.amount;
        *account_balances.entry(entry.credit.clone()).or_default() -= entry.amount;
    }

    let mut asset_balances = Vec::new();
    let mut total_assets: i64 = 0;

    let mut liability_balances = Vec::new();
    let mut total_liabilities: i64 = 0;

    // 前年の各勘定科目の定義を参照
    for acc in &prev_doc.accounts {
        let raw = account_balances.get(&acc.code).copied().unwrap_or(0);
        match acc.nature {
            AccountNature::Asset => {
                // 事業主貸は決算清算されるため繰り越さない
                if acc.code != AccountCode::Drawings {
                    let bal = raw; // 資産は借方プラス
                    if bal > 0 {
                        asset_balances.push((acc.name.clone(), bal));
                        total_assets += bal;
                    }
                }
            }
            AccountNature::Liability => {
                // 事業主借は決算清算されるため繰り越さない
                if acc.code != AccountCode::OwnerContribution {
                    let bal = -raw; // 負債は貸方プラス
                    if bal > 0 {
                        liability_balances.push((acc.name.clone(), bal));
                        total_liabilities += bal;
                    }
                }
            }
            _ => {}
        }
    }

    let prev_capital = account_balances
        .get(&AccountCode::Capital)
        .map(|v| -*v)
        .unwrap_or(0);
    let owner_contribution = account_balances
        .get(&AccountCode::OwnerContribution)
        .map(|v| -*v)
        .unwrap_or(0);
    let drawings = account_balances
        .get(&AccountCode::Drawings)
        .copied()
        .unwrap_or(0);

    // 当期純利益（青色申告特別控除前所得または青色特別控除後所得）
    // 個人事業主の青色決算書B/Sにおいては「青色特別控除前所得」で元入金を清算するのが標準（特別控除は税制上の控除であるため）
    // ※ ここでは ordinary_income_before_deduction を使用
    let net_income = blue.ordinary_income_before_deduction;

    // 翌期首元入金 = 前期末元入金 + 所得金額 + 事業主借 - 事業主貸
    let next_capital = prev_capital + net_income + owner_contribution - drawings;

    RolloverPreview {
        from_year,
        to_year,
        asset_balances,
        total_assets,
        liability_balances,
        total_liabilities,
        prev_capital,
        net_income,
        owner_contribution,
        drawings,
        next_capital,
    }
}

/// 前年度ドキュメントから翌年度の LedgerDocument を構築。
/// 期首残高仕訳（1月1日付）を自動生成し、マスタデータを引き継ぐ。
pub fn rollover_to_next_year(prev_doc: &LedgerDocument) -> LedgerDocument {
    let next_year = prev_doc.fiscal_year + 1;
    let mut next_doc = LedgerDocument::new(next_year);

    // 納税者情報・特別控除区分・科目マスタ・按分ルールを引き継ぎ
    next_doc.taxpayer = prev_doc.taxpayer.clone();
    next_doc.special_deduction = prev_doc.special_deduction;
    next_doc.accounts = prev_doc.accounts.clone();
    next_doc.allocation_rules = prev_doc.allocation_rules.clone();

    let opening_date = match NaiveDate::from_ymd_opt(next_year, 1, 1) {
        Some(d) => d,
        None => return next_doc,
    };

    // 補助科目ごとの残高を精密に集計して期首仕訳を生成
    // キー: (AccountCode, Option<String>) -> 借方正味金額
    let mut raw_balances: BTreeMap<(AccountCode, Option<String>), i64> = BTreeMap::new();
    for entry in &prev_doc.entries {
        *raw_balances
            .entry((entry.debit.clone(), entry.debit_sub.clone()))
            .or_default() += entry.amount;
        *raw_balances
            .entry((entry.credit.clone(), entry.credit_sub.clone()))
            .or_default() -= entry.amount;
    }

    // 1. 各資産残高の開始仕訳 (借方: 資産科目 / 貸方: 元入金)
    for ((code, sub), net) in &raw_balances {
        if code == &AccountCode::Drawings {
            continue; // 事業主貸は除外
        }
        let nature = prev_doc
            .find_account(code)
            .map(|a| a.nature)
            .unwrap_or_else(|| code.nature());

        if nature == AccountNature::Asset && *net > 0 {
            let entry = JournalEntry::new_with_sub(
                opening_date,
                code.clone(),
                sub.clone(),
                AccountCode::Capital,
                None,
                *net,
                ROLLOVER_MEMO,
            );
            next_doc.entries.push(entry);
        }
    }

    // 2. 各負債残高の開始仕訳 (借方: 元入金 / 貸方: 負債科目)
    for ((code, sub), net) in &raw_balances {
        if code == &AccountCode::OwnerContribution {
            continue; // 事業主借は除外
        }
        let nature = prev_doc
            .find_account(code)
            .map(|a| a.nature)
            .unwrap_or_else(|| code.nature());

        if nature == AccountNature::Liability && *net < 0 {
            let liability_amount = -*net;
            let entry = JournalEntry::new_with_sub(
                opening_date,
                AccountCode::Capital,
                None,
                code.clone(),
                sub.clone(),
                liability_amount,
                ROLLOVER_MEMO,
            );
            next_doc.entries.push(entry);
        }
    }

    next_doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tax_calc::BlueSpecialDeductionKind;

    #[test]
    fn test_rollover_calculation_and_opening_entries() {
        let mut prev = LedgerDocument::new(2025);
        prev.special_deduction = BlueSpecialDeductionKind::Yen650k;

        // 期首元入金 1,000,000円
        prev.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            AccountCode::Cash,
            AccountCode::Capital,
            1_000_000,
            "期首元入金",
        ));

        // 普通預金（三井住友）に売上 3,000,000円
        prev.entries.push(JournalEntry::new_with_sub(
            NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
            AccountCode::Bank,
            Some("三井住友銀行".into()),
            AccountCode::Sales,
            None,
            3_000_000,
            "売上入金",
        ));

        // 地代家賃 600,000円
        prev.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
            AccountCode::Rent,
            AccountCode::Cash,
            600_000,
            "家賃支払",
        ));

        // 生活費引出（事業主貸） 800,000円
        prev.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 8, 1).unwrap(),
            AccountCode::Drawings,
            AccountCode::Cash,
            800_000,
            "生活費",
        ));

        // 個人資金投入（事業主借） 200,000円
        prev.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 10, 1).unwrap(),
            AccountCode::Cash,
            AccountCode::OwnerContribution,
            200_000,
            "個人資金補填",
        ));

        // 買掛金 150,000円（未払仕入）
        prev.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 12, 1).unwrap(),
            AccountCode::CostOfSales,
            AccountCode::AccountsPayable,
            150_000,
            "仕入買掛",
        ));

        // プレビュー検証
        // 売上 3,000,000 - 経費(600,000+150,000=750,000) = 所得 2,250,000円
        // 現金残高: 1,000,000 - 600,000 - 800,000 + 200,000 = -200,000 (マイナス例)
        // 預金残高: 3,000,000
        // 買掛金残高: 150,000
        let preview = calculate_rollover_preview(&prev);
        assert_eq!(preview.from_year, 2025);
        assert_eq!(preview.to_year, 2026);
        assert_eq!(preview.net_income, 2_250_000);
        assert_eq!(preview.prev_capital, 1_000_000);
        assert_eq!(preview.owner_contribution, 200_000);
        assert_eq!(preview.drawings, 800_000);

        // 翌期首元入金 = 1,000,000 + 2,250,000 + 200,000 - 800,000 = 2,650,000円
        assert_eq!(preview.next_capital, 2_650_000);

        // 翌年度ドキュメント構築
        let next_doc = rollover_to_next_year(&prev);
        assert_eq!(next_doc.fiscal_year, 2026);
        assert_eq!(next_doc.special_deduction, BlueSpecialDeductionKind::Yen650k);

        // 開始仕訳の存在確認
        assert!(!next_doc.entries.is_empty());
        for e in &next_doc.entries {
            assert_eq!(e.date, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
            assert_eq!(e.description, ROLLOVER_MEMO);
        }

        // 普通預金（三井住友）が正しく期首残高として計上されているか
        let bank_entry = next_doc.entries.iter().find(|e| e.debit == AccountCode::Bank).unwrap();
        assert_eq!(bank_entry.amount, 3_000_000);
        assert_eq!(bank_entry.debit_sub.as_deref(), Some("三井住友銀行"));
        assert_eq!(bank_entry.credit, AccountCode::Capital);

        // 買掛金が正しく期首残高として計上されているか
        let ap_entry = next_doc.entries.iter().find(|e| e.credit == AccountCode::AccountsPayable).unwrap();
        assert_eq!(ap_entry.amount, 150_000);
        assert_eq!(ap_entry.debit, AccountCode::Capital);
    }
}

