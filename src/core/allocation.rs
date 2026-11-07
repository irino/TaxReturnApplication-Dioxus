//! Home-office expense allocation (家事按分) engine.
//!
//! Pure accounting logic for splitting mixed-use expenses (rent, utilities, communications, etc.)
//! into business expense and owner drawings (事業主貸) according to configured business use ratios.

use crate::core::journal::{AccountCode, JournalEntry, LedgerDocument};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// プレビューや集計用の一件の按分結果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationCalculation {
    pub account: AccountCode,
    pub account_label: String,
    pub sub_account: Option<String>,
    pub business_ratio_percent: u8,
    /// 按分前の年間経費総額
    pub total_expense: i64,
    /// 算定された事業経費額
    pub business_expense: i64,
    /// 算定された個人負担額（事業主貸に振り替える金額）
    pub personal_expense: i64,
}

const ALLOCATION_MEMO_PREFIX: &str = "家事按分振替";

/// 該当仕訳が家事按分振替仕訳かどうかを判定。
pub fn is_allocation_entry(entry: &JournalEntry) -> bool {
    entry.description.starts_with(ALLOCATION_MEMO_PREFIX)
        && entry.debit == AccountCode::Drawings
}

/// 対象年度の仕訳データとルールから、各科目の按分計算を行う。
pub fn calculate_allocations(doc: &LedgerDocument) -> Vec<AllocationCalculation> {
    let mut results = Vec::new();

    for rule in &doc.allocation_rules {
        // 家事按分振替仕訳を除外して、純粋な経費発生額を集計
        let mut total_expense: i64 = 0;
        for e in &doc.entries {
            if is_allocation_entry(e) {
                continue;
            }

            // 補助科目が指定されている場合は補助科目も一致する必要がある
            let debit_match = e.debit == rule.account
                && match &rule.sub_account {
                    Some(sub) => e.debit_sub.as_deref() == Some(sub.as_str()),
                    None => true,
                };
            let credit_match = e.credit == rule.account
                && match &rule.sub_account {
                    Some(sub) => e.credit_sub.as_deref() == Some(sub.as_str()),
                    None => true,
                };

            if debit_match {
                total_expense += e.amount;
            }
            if credit_match {
                total_expense -= e.amount;
            }
        }

        let total = total_expense.max(0);
        let ratio = rule.business_ratio_percent.min(100) as i64;
        let business_expense = total * ratio / 100;
        let personal_expense = total - business_expense;

        results.push(AllocationCalculation {
            account: rule.account.clone(),
            account_label: rule.account.label_ja().to_string(),
            sub_account: rule.sub_account.clone(),
            business_ratio_percent: rule.business_ratio_percent,
            total_expense: total,
            business_expense,
            personal_expense,
        });
    }

    results
}

/// 家事按分ルールに基づき、12月31日付の振替仕訳（借方: 事業主貸 / 貸方: 各科目）を生成。
pub fn generate_allocation_entries(doc: &LedgerDocument) -> Vec<JournalEntry> {
    let year = doc.fiscal_year;
    let end_of_year = match NaiveDate::from_ymd_opt(year, 12, 31) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let calcs = calculate_allocations(doc);
    let mut entries = Vec::new();

    for calc in calcs {
        if calc.personal_expense <= 0 {
            continue;
        }

        let desc = match &calc.sub_account {
            Some(sub) => format!(
                "{}（{} / 事業割合 {}%）",
                ALLOCATION_MEMO_PREFIX, sub, calc.business_ratio_percent
            ),
            None => format!(
                "{}（{} / 事業割合 {}%）",
                ALLOCATION_MEMO_PREFIX, calc.account_label, calc.business_ratio_percent
            ),
        };

        let entry = JournalEntry::new_with_sub(
            end_of_year,
            AccountCode::Drawings,
            None,
            calc.account,
            calc.sub_account,
            calc.personal_expense,
            desc,
        );
        entries.push(entry);
    }

    entries
}

/// 既存の家事按分仕訳を削除し、最新の計算結果に基づき家事按分仕訳を再登録。
pub fn apply_allocations(doc: &mut LedgerDocument) -> usize {
    // 既存の家事按分仕訳を削除
    doc.entries.retain(|e| !is_allocation_entry(e));

    // 新規仕訳を生成して追加
    let new_entries = generate_allocation_entries(doc);
    let count = new_entries.len();
    doc.entries.extend(new_entries);
    count
}

/// 既存の家事按分仕訳をすべてクリア。
pub fn clear_allocations(doc: &mut LedgerDocument) -> usize {
    let before = doc.entries.len();
    doc.entries.retain(|e| !is_allocation_entry(e));
    before - doc.entries.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::journal::AllocationRule;

    #[test]
    fn test_allocation_calculation_and_entry_generation() {
        let mut doc = LedgerDocument::new(2025);
        // 家賃 100,000円/月 x 12 = 1,200,000円
        for m in 1..=12 {
            doc.entries.push(JournalEntry::new(
                NaiveDate::from_ymd_opt(2025, m, 25).unwrap(),
                AccountCode::Rent,
                AccountCode::Bank,
                100_000,
                "家賃",
            ));
        }

        // 水道光熱費 20,000円
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 2, 10).unwrap(),
            AccountCode::Utilities,
            AccountCode::Cash,
            20_000,
            "電気代",
        ));

        // 按分ルール: 家賃 50% 事業割合、水道光熱費 40% 事業割合
        doc.allocation_rules.push(AllocationRule {
            account: AccountCode::Rent,
            business_ratio_percent: 50,
            sub_account: None,
        });
        doc.allocation_rules.push(AllocationRule {
            account: AccountCode::Utilities,
            business_ratio_percent: 40,
            sub_account: None,
        });

        let calcs = calculate_allocations(&doc);
        assert_eq!(calcs.len(), 2);

        // Rent: total 1,200,000, biz 600,000, personal 600,000
        assert_eq!(calcs[0].total_expense, 1_200_000);
        assert_eq!(calcs[0].business_expense, 600_000);
        assert_eq!(calcs[0].personal_expense, 600_000);

        // Utilities: total 20,000, biz 8,000 (40%), personal 12,000 (60%)
        assert_eq!(calcs[1].total_expense, 20_000);
        assert_eq!(calcs[1].business_expense, 8_000);
        assert_eq!(calcs[1].personal_expense, 12_000);

        // Apply allocations
        let count = apply_allocations(&mut doc);
        assert_eq!(count, 2);

        let alloc_entries: Vec<_> = doc.entries.iter().filter(|e| is_allocation_entry(e)).collect();
        assert_eq!(alloc_entries.len(), 2);
        assert_eq!(alloc_entries[0].date, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
        assert_eq!(alloc_entries[0].debit, AccountCode::Drawings);
        assert_eq!(alloc_entries[0].credit, AccountCode::Rent);
        assert_eq!(alloc_entries[0].amount, 600_000);

        // Re-applying does not duplicate
        let count2 = apply_allocations(&mut doc);
        assert_eq!(count2, 2);
        let alloc_entries2: Vec<_> = doc.entries.iter().filter(|e| is_allocation_entry(e)).collect();
        assert_eq!(alloc_entries2.len(), 2);

        // Clear
        let cleared = clear_allocations(&mut doc);
        assert_eq!(cleared, 2);
        assert!(!doc.entries.iter().any(|e| is_allocation_entry(e)));
    }
}
