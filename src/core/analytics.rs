//! Business Analytics, Financial Trends & Cashflow Metrics (経営分析・月次損益・資金繰り分析).
//!
//! Provides comprehensive managerial metrics computed purely from ledger entries:
//! 1. Monthly revenue, COGS, gross profit, expenses, and net operating income (1〜12月)
//! 2. Monthly cash inflow, outflow, and net cashflow trends
//! 3. Key Performance Indicators (KPIs): Gross Margin %, Operating Margin %, Cash on hand
//! 4. Itemized expense breakdown shares (構成比率 %) and ranking
//! 5. Quarterly aggregations (Q1〜Q4)

use crate::core::journal::{AccountCode, AccountNature, LedgerDocument};
use chrono::Datelike;
use serde::{Deserialize, Serialize};

/// 月次財務メトリクス（1〜12月）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyMetric {
    /// 月 (1〜12)
    pub month: u32,
    /// 売上高（収入金額）
    pub revenue: i64,
    /// 売上原価（仕入）
    pub cogs: i64,
    /// 売上総利益（粗利: 売上高 - 売上原価）
    pub gross_profit: i64,
    /// 経費合計（販管費）
    pub expenses: i64,
    /// 営業所得（差引純利益: 売上総利益 - 経費合計）
    pub net_income: i64,
    /// 現預金流入額（現金・普通預金の増加取引）
    pub cash_inflow: i64,
    /// 現預金流出額（現金・普通預金の減少取引）
    pub cash_outflow: i64,
    /// 正味キャッシュフロー（流入 - 流出）
    pub cash_net: i64,
}

impl MonthlyMetric {
    pub fn new(month: u32) -> Self {
        Self {
            month,
            revenue: 0,
            cogs: 0,
            gross_profit: 0,
            expenses: 0,
            net_income: 0,
            cash_inflow: 0,
            cash_outflow: 0,
            cash_net: 0,
        }
    }
}

/// 経費科目別の年間構成比率
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpenseShare {
    pub account: AccountCode,
    pub account_name: String,
    pub amount: i64,
    /// 全経費に対する構成割合（0.0〜100.0%）
    pub percentage: f64,
}

/// 主要経営財務KPI（健全性指標）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinancialKPIs {
    /// 年間総売上高
    pub annual_revenue: i64,
    /// 年間売上原価
    pub annual_cogs: i64,
    /// 年間売上総利益（粗利）
    pub gross_profit: i64,
    /// 年間総経費
    pub annual_expenses: i64,
    /// 青色申告特別控除前事業所得（当期純利益）
    pub net_business_income: i64,
    /// 売上総利益率（粗利率 %）: gross_profit / annual_revenue
    pub gross_margin_percent: f64,
    /// 営業利益率（%）: net_business_income / annual_revenue
    pub operating_margin_percent: f64,
    /// 期末現預金残高（現金 + 普通預金）
    pub cash_and_bank_balance: i64,
    /// 期末売掛金残高
    pub accounts_receivable: i64,
    /// 期末買掛金残高
    pub accounts_payable: i64,
    /// 最大経費科目（科目名, 年間金額）
    pub largest_expense: Option<(String, i64)>,
}

/// 経営分析総合レポート
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsSummary {
    pub fiscal_year: i32,
    pub kpis: FinancialKPIs,
    /// 月別推移（1月〜12月の12要素）
    pub monthly_metrics: Vec<MonthlyMetric>,
    /// 経費構成比ランキング（金額降順）
    pub expense_shares: Vec<ExpenseShare>,
    /// 四半期別売上高 [Q1, Q2, Q3, Q4]
    pub quarterly_revenue: [i64; 4],
    /// 四半期別純所得 [Q1, Q2, Q3, Q4]
    pub quarterly_income: [i64; 4],
}

/// 仕訳帳データから経営分析サマリーを計算
pub fn calculate_analytics(doc: &LedgerDocument) -> AnalyticsSummary {
    let mut monthly = (1..=12).map(MonthlyMetric::new).collect::<Vec<_>>();
    let mut expense_map = std::collections::HashMap::<AccountCode, i64>::new();

    let mut total_cash_and_bank = 0i64;
    let mut total_receivables = 0i64;
    let mut total_payables = 0i64;

    for entry in &doc.entries {
        let m = entry.date.month() as usize;
        let month_idx = if (1..=12).contains(&m) { m - 1 } else { 0 };

        // 売上高 (貸方 Sales または収益)
        if entry.credit == AccountCode::Sales || entry.credit.nature() == AccountNature::Income {
            monthly[month_idx].revenue += entry.amount;
        }

        // 売上原価 (借方 CostOfSales)
        if entry.debit == AccountCode::CostOfSales {
            monthly[month_idx].cogs += entry.amount;
        }

        // 経費 (借方 費用科目、CostOfSales以外)
        if entry.debit.nature() == AccountNature::Expense && entry.debit != AccountCode::CostOfSales {
            monthly[month_idx].expenses += entry.amount;
            *expense_map.entry(entry.debit.clone()).or_insert(0) += entry.amount;
        }

        // 現預金の増減（借方で増加、貸方で減少）
        let is_cash_debit = matches!(entry.debit, AccountCode::Cash | AccountCode::Bank);
        let is_cash_credit = matches!(entry.credit, AccountCode::Cash | AccountCode::Bank);
        if is_cash_debit && !is_cash_credit {
            monthly[month_idx].cash_inflow += entry.amount;
            total_cash_and_bank += entry.amount;
        } else if is_cash_credit && !is_cash_debit {
            monthly[month_idx].cash_outflow += entry.amount;
            total_cash_and_bank -= entry.amount;
        }

        // 売掛金 (借方で増加、貸方で減少)
        if entry.debit == AccountCode::AccountsReceivable {
            total_receivables += entry.amount;
        }
        if entry.credit == AccountCode::AccountsReceivable {
            total_receivables -= entry.amount;
        }

        // 買掛金 (貸方で増加、借方で減少)
        if entry.credit == AccountCode::AccountsPayable {
            total_payables += entry.amount;
        }
        if entry.debit == AccountCode::AccountsPayable {
            total_payables -= entry.amount;
        }
    }

    // 月次の粗利・純利益・正味キャッシュフローを算出
    for m in &mut monthly {
        m.gross_profit = m.revenue - m.cogs;
        m.net_income = m.gross_profit - m.expenses;
        m.cash_net = m.cash_inflow - m.cash_outflow;
    }

    let annual_revenue: i64 = monthly.iter().map(|m| m.revenue).sum();
    let annual_cogs: i64 = monthly.iter().map(|m| m.cogs).sum();
    let gross_profit = annual_revenue - annual_cogs;
    let annual_expenses: i64 = monthly.iter().map(|m| m.expenses).sum();
    let net_business_income = gross_profit - annual_expenses;

    let gross_margin_percent = if annual_revenue > 0 {
        (gross_profit as f64 / annual_revenue as f64) * 100.0
    } else {
        0.0
    };

    let operating_margin_percent = if annual_revenue > 0 {
        (net_business_income as f64 / annual_revenue as f64) * 100.0
    } else {
        0.0
    };

    // 経費構成比ランキングの構築
    let mut expense_shares = Vec::new();
    for (acc, amount) in expense_map {
        let pct = if annual_expenses > 0 {
            (amount as f64 / annual_expenses as f64) * 100.0
        } else {
            0.0
        };
        let acc_name = doc
            .accounts
            .iter()
            .find(|a| a.code == acc)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| acc.label_ja().to_string());
        expense_shares.push(ExpenseShare {
            account: acc,
            account_name: acc_name,
            amount,
            percentage: (pct * 10.0).round() / 10.0,
        });
    }
    expense_shares.sort_by(|a, b| b.amount.cmp(&a.amount));

    let largest_expense = expense_shares
        .first()
        .map(|s| (s.account_name.clone(), s.amount));

    // 四半期別集計
    let q1_rev = monthly[0..3].iter().map(|m| m.revenue).sum();
    let q2_rev = monthly[3..6].iter().map(|m| m.revenue).sum();
    let q3_rev = monthly[6..9].iter().map(|m| m.revenue).sum();
    let q4_rev = monthly[9..12].iter().map(|m| m.revenue).sum();

    let q1_inc = monthly[0..3].iter().map(|m| m.net_income).sum();
    let q2_inc = monthly[3..6].iter().map(|m| m.net_income).sum();
    let q3_inc = monthly[6..9].iter().map(|m| m.net_income).sum();
    let q4_inc = monthly[9..12].iter().map(|m| m.net_income).sum();

    let kpis = FinancialKPIs {
        annual_revenue,
        annual_cogs,
        gross_profit,
        annual_expenses,
        net_business_income,
        gross_margin_percent: (gross_margin_percent * 10.0).round() / 10.0,
        operating_margin_percent: (operating_margin_percent * 10.0).round() / 10.0,
        cash_and_bank_balance: total_cash_and_bank,
        accounts_receivable: total_receivables,
        accounts_payable: total_payables,
        largest_expense,
    };

    AnalyticsSummary {
        fiscal_year: doc.fiscal_year,
        kpis,
        monthly_metrics: monthly,
        expense_shares,
        quarterly_revenue: [q1_rev, q2_rev, q3_rev, q4_rev],
        quarterly_income: [q1_inc, q2_inc, q3_inc, q4_inc],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::journal::JournalEntry;
    use chrono::NaiveDate;

    #[test]
    fn test_monthly_and_kpi_calculation() {
        let mut doc = LedgerDocument::new(2026);
        // 1月売上 1,000,000円
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            1_000_000,
            "1月売上",
        ));
        // 1月経費 (消耗品 200,000円)
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2026, 1, 20).unwrap(),
            AccountCode::Supplies,
            AccountCode::Bank,
            200_000,
            "1月PC購入",
        ));
        // 2月売上 2,000,000円
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            2_000_000,
            "2月売上",
        ));
        // 2月経費 (通信費 100,000円)
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(),
            AccountCode::Communication,
            AccountCode::Bank,
            100_000,
            "2月通信費",
        ));

        let summary = calculate_analytics(&doc);
        assert_eq!(summary.kpis.annual_revenue, 3_000_000);
        assert_eq!(summary.kpis.annual_expenses, 300_000);
        assert_eq!(summary.kpis.net_business_income, 2_700_000);
        assert_eq!(summary.kpis.gross_margin_percent, 100.0);
        assert_eq!(summary.kpis.operating_margin_percent, 90.0);

        // 1月のメトリクス
        assert_eq!(summary.monthly_metrics[0].revenue, 1_000_000);
        assert_eq!(summary.monthly_metrics[0].expenses, 200_000);
        assert_eq!(summary.monthly_metrics[0].net_income, 800_000);
        assert_eq!(summary.monthly_metrics[0].cash_net, 800_000);

        // 2月のメトリクス
        assert_eq!(summary.monthly_metrics[1].revenue, 2_000_000);
        assert_eq!(summary.monthly_metrics[1].expenses, 100_000);
        assert_eq!(summary.monthly_metrics[1].net_income, 1_900_000);

        // 経費構成比ランキング: 消耗品200k (66.7%), 通信費100k (33.3%)
        assert_eq!(summary.expense_shares.len(), 2);
        assert_eq!(summary.expense_shares[0].account, AccountCode::Supplies);
        assert_eq!(summary.expense_shares[0].amount, 200_000);
        assert_eq!(summary.expense_shares[0].percentage, 66.7);
        assert_eq!(summary.expense_shares[1].account, AccountCode::Communication);
        assert_eq!(summary.expense_shares[1].amount, 100_000);
        assert_eq!(summary.expense_shares[1].percentage, 33.3);
    }
}
