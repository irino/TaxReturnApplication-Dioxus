//! Aggregation and tax estimation for 青色申告決算書 / 確定申告書B.
//! Pure functions only — no I/O, no UI.

use crate::core::journal::{AccountCode, AccountNature, JournalEntry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 青色申告特別控除の区分（簡易）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlueSpecialDeductionKind {
    /// 控除なし
    None,
    /// 10万円控除（簡易簿記等）
    #[default]
    Yen100k,
    /// 65万円控除（複式簿記 + e-Tax 等の要件を満たす前提の選択値）
    Yen650k,
}

impl BlueSpecialDeductionKind {
    pub fn amount(self) -> i64 {
        match self {
            Self::None => 0,
            Self::Yen100k => 100_000,
            Self::Yen650k => 650_000,
        }
    }

    pub fn label_ja(self) -> &'static str {
        match self {
            Self::None => "なし（0円）",
            Self::Yen100k => "10万円控除",
            Self::Yen650k => "65万円控除",
        }
    }

    pub fn all() -> &'static [BlueSpecialDeductionKind] {
        &[Self::None, Self::Yen100k, Self::Yen650k]
    }
}

/// 経費の内訳（青色申告決算書の経費欄に相当）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExpenseBreakdown {
    pub travel: i64,
    pub communication: i64,
    pub supplies: i64,
    pub rent: i64,
    pub utilities: i64,
    pub miscellaneous: i64,
}

impl ExpenseBreakdown {
    pub fn total(&self) -> i64 {
        self.travel
            + self.communication
            + self.supplies
            + self.rent
            + self.utilities
            + self.miscellaneous
    }

    pub fn lines(&self) -> Vec<(&'static str, i64)> {
        vec![
            ("旅費交通費", self.travel),
            ("通信費", self.communication),
            ("消耗品費", self.supplies),
            ("地代家賃", self.rent),
            ("水道光熱費", self.utilities),
            ("雑費", self.miscellaneous),
        ]
    }
}

/// 青色申告決算書向け集計。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BlueReturnSummary {
    pub fiscal_year: i32,
    pub sales: i64,
    pub cost_of_sales: i64,
    /// 売上総利益 = 売上 − 仕入
    pub gross_profit: i64,
    /// 従来型経費内訳（基本6科目）
    pub expenses: ExpenseBreakdown,
    /// 追加標準科目・カスタム科目を含む全経費の内訳（科目名, 金額）
    #[serde(default)]
    pub itemized_expenses: Vec<(String, i64)>,
    /// 全経費の総合計（売上原価を除くすべての費用）
    pub total_expenses: i64,
    /// 差引金額（特別控除前）= 売上総利益 − 経費合計
    pub ordinary_income_before_deduction: i64,
    pub special_deduction_kind: BlueSpecialDeductionKind,
    pub special_deduction: i64,
    /// 事業所得（特別控除後、負なら0）
    pub business_income: i64,
    /// 科目別残高（表示用）
    pub by_account: BTreeMap<String, i64>,
    // 貸借対照表の簡易残高
    pub cash: i64,
    pub bank: i64,
    pub accounts_receivable: i64,
    pub accounts_payable: i64,
    pub capital: i64,
    pub drawings: i64,
    pub owner_contribution: i64,
}

/// 確定申告書B向けの所得・税額の概算。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FormBSummary {
    pub fiscal_year: i32,
    /// 事業所得（青色特別控除後）
    pub business_income: i64,
    /// 所得金額合計（現状は事業所得のみ）
    pub total_income: i64,
    /// 基礎控除（令和2年分以降の原則 48万円）
    pub basic_deduction: i64,
    /// 所得控除合計（現状は基礎控除のみ）
    pub total_deductions: i64,
    /// 課税所得（千円未満切捨て後）
    pub taxable_income: i64,
    /// 所得税額（百円未満切捨て）
    pub income_tax: i64,
    /// 復興特別所得税（所得税 × 2.1%）
    pub reconstruction_surtax: i64,
    /// 所得税 + 復興特別所得税
    pub total_income_tax: i64,
}

/// 帳票一式の集計結果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxReportBundle {
    pub blue: BlueReturnSummary,
    pub form_b: FormBSummary,
}

/// Debit-positive signed movement per account, then adjusted by nature.
pub fn aggregate_account_totals(entries: &[JournalEntry]) -> BTreeMap<AccountCode, i64> {
    let mut raw: BTreeMap<AccountCode, i64> = BTreeMap::new();
    for entry in entries {
        *raw.entry(entry.debit.clone()).or_default() += entry.amount;
        *raw.entry(entry.credit.clone()).or_default() -= entry.amount;
    }

    let mut out = BTreeMap::new();
    for (code, signed) in raw {
        let reported = match code.nature() {
            AccountNature::Asset | AccountNature::Expense => signed,
            AccountNature::Liability | AccountNature::Equity | AccountNature::Income => -signed,
        };
        out.insert(code, reported);
    }
    out
}

fn bal(totals: &BTreeMap<AccountCode, i64>, code: &AccountCode) -> i64 {
    totals.get(code).copied().unwrap_or(0)
}

/// 青色申告決算書相当の集計。
pub fn aggregate_blue_return(
    entries: &[JournalEntry],
    fiscal_year: i32,
    special_deduction: BlueSpecialDeductionKind,
) -> BlueReturnSummary {
    let totals = aggregate_account_totals(entries);

    let sales = bal(&totals, &AccountCode::Sales);
    let cost_of_sales = bal(&totals, &AccountCode::CostOfSales);
    let gross_profit = sales - cost_of_sales;

    let expenses = ExpenseBreakdown {
        travel: bal(&totals, &AccountCode::Travel),
        communication: bal(&totals, &AccountCode::Communication),
        supplies: bal(&totals, &AccountCode::Supplies),
        rent: bal(&totals, &AccountCode::Rent),
        utilities: bal(&totals, &AccountCode::Utilities),
        miscellaneous: bal(&totals, &AccountCode::Miscellaneous),
    };

    // すべての費用科目（売上原価を除く）を収集・集計
    let mut itemized_expenses: Vec<(String, i64)> = Vec::new();
    let mut total_expenses: i64 = 0;

    for (code, amount) in &totals {
        if code.nature() == AccountNature::Expense && code != &AccountCode::CostOfSales {
            if *amount > 0 {
                itemized_expenses.push((code.label_ja().to_string(), *amount));
            }
            total_expenses += *amount;
        }
    }
    // 金額降順にソート
    itemized_expenses.sort_by(|a, b| b.1.cmp(&a.1));

    let ordinary_income_before_deduction = gross_profit - total_expenses;

    let special_amt = special_deduction.amount();
    // 特別控除は所得を超えて適用しない
    let applied_special = special_amt.min(ordinary_income_before_deduction.max(0));
    let business_income = (ordinary_income_before_deduction - applied_special).max(0);

    let by_account = totals
        .iter()
        .map(|(code, amount)| (code.label_ja().to_string(), *amount))
        .collect();

    BlueReturnSummary {
        fiscal_year,
        sales,
        cost_of_sales,
        gross_profit,
        expenses,
        itemized_expenses,
        total_expenses,
        ordinary_income_before_deduction,
        special_deduction_kind: special_deduction,
        special_deduction: applied_special,
        business_income,
        by_account,
        cash: bal(&totals, &AccountCode::Cash),
        bank: bal(&totals, &AccountCode::Bank),
        accounts_receivable: bal(&totals, &AccountCode::AccountsReceivable),
        accounts_payable: bal(&totals, &AccountCode::AccountsPayable),
        capital: bal(&totals, &AccountCode::Capital),
        drawings: bal(&totals, &AccountCode::Drawings),
        owner_contribution: bal(&totals, &AccountCode::OwnerContribution),
    }
}

/// 基礎控除（合計所得 2,400万円以下の原則額）。
pub const BASIC_DEDUCTION_DEFAULT: i64 = 480_000;

/// 課税所得を算出（所得 − 所得控除、負なら0、千円未満切捨て）。
pub fn compute_taxable_income(total_income: i64, total_deductions: i64) -> i64 {
    let raw = (total_income - total_deductions).max(0);
    // 国税通則: 課税所得は千円未満切捨て
    (raw / 1_000) * 1_000
}

/// 所得税の速算表（令和5年分以降の一般的な税率）。
/// 戻り値は百円未満切捨て前の税額。
pub fn compute_income_tax_before_rounding(taxable_income: i64) -> i64 {
    if taxable_income <= 0 {
        return 0;
    }
    let tax = if taxable_income <= 1_949_000 {
        taxable_income * 5 / 100
    } else if taxable_income <= 3_299_000 {
        taxable_income * 10 / 100 - 97_500
    } else if taxable_income <= 6_949_000 {
        taxable_income * 20 / 100 - 427_500
    } else if taxable_income <= 8_999_000 {
        taxable_income * 23 / 100 - 636_000
    } else if taxable_income <= 17_999_000 {
        taxable_income * 33 / 100 - 1_536_000
    } else if taxable_income <= 39_999_000 {
        taxable_income * 40 / 100 - 2_796_000
    } else {
        taxable_income * 45 / 100 - 4_796_000
    };
    tax.max(0)
}

/// 百円未満切捨て。
pub fn floor_to_hundred(yen: i64) -> i64 {
    (yen / 100) * 100
}

/// 復興特別所得税 = 基準所得税額 × 2.1%（1円未満切捨て）。
pub fn compute_reconstruction_surtax(income_tax: i64) -> i64 {
    income_tax * 21 / 1000
}

/// 確定申告書B相当の概算。
pub fn aggregate_form_b(blue: &BlueReturnSummary) -> FormBSummary {
    let business_income = blue.business_income;
    let total_income = business_income;
    let basic_deduction = if total_income <= 24_000_000 {
        BASIC_DEDUCTION_DEFAULT
    } else if total_income <= 24_500_000 {
        320_000
    } else if total_income <= 25_000_000 {
        160_000
    } else {
        0
    };
    let total_deductions = basic_deduction;
    let taxable_income = compute_taxable_income(total_income, total_deductions);
    let income_tax = floor_to_hundred(compute_income_tax_before_rounding(taxable_income));
    let reconstruction_surtax = compute_reconstruction_surtax(income_tax);
    let total_income_tax = income_tax + reconstruction_surtax;

    FormBSummary {
        fiscal_year: blue.fiscal_year,
        business_income,
        total_income,
        basic_deduction,
        total_deductions,
        taxable_income,
        income_tax,
        reconstruction_surtax,
        total_income_tax,
    }
}

/// 仕訳から帳票一式を集計。
pub fn build_tax_report(
    entries: &[JournalEntry],
    fiscal_year: i32,
    special_deduction: BlueSpecialDeductionKind,
) -> TaxReportBundle {
    let blue = aggregate_blue_return(entries, fiscal_year, special_deduction);
    let form_b = aggregate_form_b(&blue);
    TaxReportBundle { blue, form_b }
}

// ---- Backward-compatible helper used by older call sites / tests ----

/// Convenience wrapper: 10万円控除で青色集計（旧 API 互換）。
pub fn aggregate_blue_return_default(
    entries: &[JournalEntry],
    fiscal_year: i32,
) -> BlueReturnSummary {
    aggregate_blue_return(entries, fiscal_year, BlueSpecialDeductionKind::Yen100k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn entry(
        month: u32,
        day: u32,
        debit: AccountCode,
        credit: AccountCode,
        amount: i64,
        desc: &str,
    ) -> JournalEntry {
        JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, month, day).unwrap(),
            debit,
            credit,
            amount,
            desc,
        )
    }

    fn sample_entries() -> Vec<JournalEntry> {
        vec![
            entry(1, 10, AccountCode::Cash, AccountCode::Sales, 100_000, "売上"),
            entry(
                2,
                1,
                AccountCode::Bank,
                AccountCode::Sales,
                50_000,
                "売上振込",
            ),
            entry(
                2,
                5,
                AccountCode::Travel,
                AccountCode::Cash,
                3_000,
                "交通費",
            ),
            entry(3, 1, AccountCode::Rent, AccountCode::Bank, 20_000, "家賃"),
            entry(
                3,
                15,
                AccountCode::CostOfSales,
                AccountCode::AccountsPayable,
                40_000,
                "仕入",
            ),
        ]
    }

    #[test]
    fn sales_and_expense_summary() {
        let summary =
            aggregate_blue_return(&sample_entries(), 2025, BlueSpecialDeductionKind::None);
        assert_eq!(summary.sales, 150_000);
        assert_eq!(summary.cost_of_sales, 40_000);
        assert_eq!(summary.gross_profit, 110_000);
        assert_eq!(summary.total_expenses, 23_000);
        assert_eq!(summary.ordinary_income_before_deduction, 87_000);
        assert_eq!(summary.special_deduction, 0);
        assert_eq!(summary.business_income, 87_000);
        assert_eq!(summary.expenses.travel, 3_000);
        assert_eq!(summary.expenses.rent, 20_000);
    }

    #[test]
    fn special_deduction_capped_by_income() {
        let summary =
            aggregate_blue_return(&sample_entries(), 2025, BlueSpecialDeductionKind::Yen100k);
        assert_eq!(summary.ordinary_income_before_deduction, 87_000);
        assert_eq!(summary.special_deduction, 87_000); // min(100k, 87k)
        assert_eq!(summary.business_income, 0);
    }

    #[test]
    fn special_deduction_650k_on_larger_income() {
        let entries = vec![entry(
            6,
            1,
            AccountCode::Bank,
            AccountCode::Sales,
            2_000_000,
            "売上",
        )];
        let summary = aggregate_blue_return(&entries, 2025, BlueSpecialDeductionKind::Yen650k);
        assert_eq!(summary.ordinary_income_before_deduction, 2_000_000);
        assert_eq!(summary.special_deduction, 650_000);
        assert_eq!(summary.business_income, 1_350_000);
    }

    #[test]
    fn form_b_tax_brackets() {
        // 事業所得 1,350,000、基礎控除 480,000 → 課税所得 870,000
        // 税額 870,000 * 5% = 43,500 → 百円未満切捨て 43,500
        let entries = vec![entry(
            6,
            1,
            AccountCode::Bank,
            AccountCode::Sales,
            2_000_000,
            "売上",
        )];
        let report = build_tax_report(&entries, 2025, BlueSpecialDeductionKind::Yen650k);
        assert_eq!(report.form_b.business_income, 1_350_000);
        assert_eq!(report.form_b.basic_deduction, 480_000);
        assert_eq!(report.form_b.taxable_income, 870_000);
        assert_eq!(report.form_b.income_tax, 43_500);
        assert_eq!(report.form_b.reconstruction_surtax, 43_500 * 21 / 1000);
        assert_eq!(
            report.form_b.total_income_tax,
            report.form_b.income_tax + report.form_b.reconstruction_surtax
        );
    }

    #[test]
    fn taxable_income_floors_to_thousand() {
        assert_eq!(compute_taxable_income(1_000_499, 0), 1_000_000);
        assert_eq!(compute_taxable_income(100, 0), 0);
    }

    #[test]
    fn income_tax_second_bracket() {
        // 課税所得 2,000,000 → 10% - 97,500 = 102,500
        assert_eq!(compute_income_tax_before_rounding(2_000_000), 102_500);
        assert_eq!(floor_to_hundred(102_599), 102_500);
    }

    #[test]
    fn income_tax_all_brackets() {
        assert_eq!(compute_income_tax_before_rounding(-500), 0);
        assert_eq!(compute_income_tax_before_rounding(0), 0);
        // Bracket 1: 5%
        assert_eq!(compute_income_tax_before_rounding(1_000_000), 50_000);
        // Bracket 3: 20% - 427,500
        assert_eq!(compute_income_tax_before_rounding(5_000_000), 572_500);
        // Bracket 4: 23% - 636,000
        assert_eq!(compute_income_tax_before_rounding(8_000_000), 1_204_000);
        // Bracket 5: 33% - 1,536,000
        assert_eq!(compute_income_tax_before_rounding(10_000_000), 1_764_000);
        // Bracket 6: 40% - 2,796,000
        assert_eq!(compute_income_tax_before_rounding(20_000_000), 5_204_000);
        // Bracket 7: 45% - 4,796,000
        assert_eq!(compute_income_tax_before_rounding(50_000_000), 17_704_000);
    }

    #[test]
    fn basic_deduction_tiers_for_high_earners() {
        let mut blue = BlueReturnSummary::default();
        blue.business_income = 24_000_000;
        assert_eq!(aggregate_form_b(&blue).basic_deduction, 480_000);

        blue.business_income = 24_300_000;
        assert_eq!(aggregate_form_b(&blue).basic_deduction, 320_000);

        blue.business_income = 24_800_000;
        assert_eq!(aggregate_form_b(&blue).basic_deduction, 160_000);

        blue.business_income = 26_000_000;
        assert_eq!(aggregate_form_b(&blue).basic_deduction, 0);
    }

    #[test]
    fn balance_sheet_account_balances() {
        let entries = vec![
            entry(1, 1, AccountCode::Cash, AccountCode::Capital, 1_000_000, "開業元入金"),
            entry(1, 10, AccountCode::Bank, AccountCode::Cash, 800_000, "預金預け入れ"),
            entry(1, 15, AccountCode::AccountsReceivable, AccountCode::Sales, 500_000, "売掛売上"),
            entry(1, 20, AccountCode::CostOfSales, AccountCode::AccountsPayable, 200_000, "買掛仕入"),
            entry(1, 25, AccountCode::Drawings, AccountCode::Bank, 100_000, "生活費引出"),
            entry(1, 30, AccountCode::Bank, AccountCode::OwnerContribution, 50_000, "個人資金入金"),
        ];
        let summary = aggregate_blue_return(&entries, 2025, BlueSpecialDeductionKind::None);
        assert_eq!(summary.cash, 200_000); // 1,000,000 - 800,000
        assert_eq!(summary.bank, 750_000); // 800,000 - 100,000 + 50,000
        assert_eq!(summary.accounts_receivable, 500_000);
        assert_eq!(summary.accounts_payable, 200_000);
        assert_eq!(summary.capital, 1_000_000);
        assert_eq!(summary.drawings, 100_000);
        assert_eq!(summary.owner_contribution, 50_000);
    }

    #[test]
    fn empty_ledger_is_zero() {
        let summary = aggregate_blue_return(&[], 2025, BlueSpecialDeductionKind::Yen100k);
        assert_eq!(summary.fiscal_year, 2025);
        assert_eq!(summary.sales, 0);
        assert_eq!(summary.business_income, 0);
        let form_b = aggregate_form_b(&summary);
        assert_eq!(form_b.taxable_income, 0);
        assert_eq!(form_b.income_tax, 0);
    }
}

