//! Compound Transactions & Multi-Split Ledger Architecture (複合仕訳・諸口取引).
//!
//! Provides a standardized data model where a single transaction header owns two or more splits:
//! $\sum \text{debits} == \sum \text{credits}$
//!
//! Fully compatible with GnuCash's `transactions` + `splits` relational schema
//! and convertible to/from standard single-line `JournalEntry`.

use crate::core::journal::{AccountCode, JournalEntry};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CompoundError {
    #[error("仕訳明細（スプリット）は最低2行以上必要です")]
    TooFewSplits,
    #[error("借方合計（¥{debit_sum}）と貸方合計（¥{credit_sum}）が一致しません（差額: ¥{diff}）")]
    UnbalancedSplits {
        debit_sum: i64,
        credit_sum: i64,
        diff: i64,
    },
    #[error("金額は1円以上である必要があります")]
    NonPositiveAmount,
    #[error("摘要を入力してください")]
    EmptyDescription,
}

/// 複合仕訳の1明細行（スプリット）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompoundSplit {
    pub id: String,
    pub account: AccountCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_account: Option<String>,
    /// 金額（常に正の整数・円）
    pub amount: i64,
    /// 借方 (true) または 貸方 (false)
    pub is_debit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
}

impl CompoundSplit {
    pub fn debit(
        account: AccountCode,
        sub_account: Option<String>,
        amount: i64,
        memo: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            account,
            sub_account,
            amount,
            is_debit: true,
            memo,
        }
    }

    pub fn credit(
        account: AccountCode,
        sub_account: Option<String>,
        amount: i64,
        memo: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            account,
            sub_account,
            amount,
            is_debit: false,
            memo,
        }
    }

    /// GnuCash表記の符号付き金額（借方プラス、貸方マイナス）
    pub fn signed_amount(&self) -> i64 {
        if self.is_debit {
            self.amount
        } else {
            -self.amount
        }
    }
}

/// 取引ヘッダー（複数スプリットを保有する複合仕訳）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompoundTransaction {
    pub id: String,
    pub date: NaiveDate,
    pub description: String,
    pub splits: Vec<CompoundSplit>,
    pub created_at: DateTime<Local>,
}

impl CompoundTransaction {
    pub fn new(
        date: NaiveDate,
        description: impl Into<String>,
        splits: Vec<CompoundSplit>,
    ) -> Result<Self, CompoundError> {
        let desc = description.into();
        if desc.trim().is_empty() {
            return Err(CompoundError::EmptyDescription);
        }
        if splits.len() < 2 {
            return Err(CompoundError::TooFewSplits);
        }

        let mut debit_sum: i64 = 0;
        let mut credit_sum: i64 = 0;

        for s in &splits {
            if s.amount <= 0 {
                return Err(CompoundError::NonPositiveAmount);
            }
            if s.is_debit {
                debit_sum += s.amount;
            } else {
                credit_sum += s.amount;
            }
        }

        if debit_sum != credit_sum {
            return Err(CompoundError::UnbalancedSplits {
                debit_sum,
                credit_sum,
                diff: (debit_sum - credit_sum).abs(),
            });
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            date,
            description: desc,
            splits,
            created_at: Local::now(),
        })
    }

    /// 借方合計額（貸方合計額と同額）
    pub fn total_amount(&self) -> i64 {
        self.splits
            .iter()
            .filter(|s| s.is_debit)
            .map(|s| s.amount)
            .sum()
    }

    /// 単純仕訳（1借方 : 1貸方）かどうかを判定
    pub fn is_simple(&self) -> bool {
        let debits = self.splits.iter().filter(|s| s.is_debit).count();
        let credits = self.splits.iter().filter(|s| !s.is_debit).count();
        debits == 1 && credits == 1
    }

    /// 単一の JournalEntry から CompoundTransaction を生成
    pub fn from_journal_entry(entry: &JournalEntry) -> Self {
        let splits = vec![
            CompoundSplit {
                id: Uuid::new_v4().to_string(),
                account: entry.debit.clone(),
                sub_account: entry.debit_sub.clone(),
                amount: entry.amount,
                is_debit: true,
                memo: None,
            },
            CompoundSplit {
                id: Uuid::new_v4().to_string(),
                account: entry.credit.clone(),
                sub_account: entry.credit_sub.clone(),
                amount: entry.amount,
                is_debit: false,
                memo: None,
            },
        ];

        Self {
            id: entry.id.clone(),
            date: entry.date,
            description: entry.description.clone(),
            splits,
            created_at: entry.created_at,
        }
    }

    /// 複合仕訳を帳票計算・一覧表示用の JournalEntry ペア群へ展開
    /// 単純仕訳の場合はそのまま1件、複合仕訳（1:N や N:1）の場合は各明細行を分割して射影
    pub fn to_journal_entries(&self) -> Vec<JournalEntry> {
        let debits: Vec<&CompoundSplit> = self.splits.iter().filter(|s| s.is_debit).collect();
        let credits: Vec<&CompoundSplit> = self.splits.iter().filter(|s| !s.is_debit).collect();

        if debits.len() == 1 && credits.len() == 1 {
            // 単純仕訳 (1:1)
            let d = debits[0];
            let c = credits[0];
            return vec![JournalEntry {
                id: self.id.clone(),
                date: self.date,
                debit: d.account.clone(),
                debit_sub: d.sub_account.clone(),
                credit: c.account.clone(),
                credit_sub: c.sub_account.clone(),
                amount: d.amount,
                description: self.description.clone(),
                created_at: self.created_at,
            }];
        }

        let mut entries = Vec::new();
        // 1:N 構造（借方1つに対して複数の貸方）
        if debits.len() == 1 {
            let d = debits[0];
            for c in credits {
                let memo_suffix = c.memo.as_deref().map(|m| format!(" ({m})")).unwrap_or_default();
                entries.push(JournalEntry {
                    id: c.id.clone(),
                    date: self.date,
                    debit: d.account.clone(),
                    debit_sub: d.sub_account.clone(),
                    credit: c.account.clone(),
                    credit_sub: c.sub_account.clone(),
                    amount: c.amount,
                    description: format!("{}{memo_suffix}", self.description),
                    created_at: self.created_at,
                });
            }
            return entries;
        }

        // N:1 構造（複数の借方に対して貸方1つ）
        if credits.len() == 1 {
            let c = credits[0];
            for d in debits {
                let memo_suffix = d.memo.as_deref().map(|m| format!(" ({m})")).unwrap_or_default();
                entries.push(JournalEntry {
                    id: d.id.clone(),
                    date: self.date,
                    debit: d.account.clone(),
                    debit_sub: d.sub_account.clone(),
                    credit: c.account.clone(),
                    credit_sub: c.sub_account.clone(),
                    amount: d.amount,
                    description: format!("{}{memo_suffix}", self.description),
                    created_at: self.created_at,
                });
            }
            return entries;
        }

        // N:N 構造の場合は最大の金額から順にグリーディに対応付け
        let mut d_rem: Vec<(AccountCode, Option<String>, i64)> = debits
            .iter()
            .map(|d| (d.account.clone(), d.sub_account.clone(), d.amount))
            .collect();
        let mut c_rem: Vec<(AccountCode, Option<String>, i64)> = credits
            .iter()
            .map(|c| (c.account.clone(), c.sub_account.clone(), c.amount))
            .collect();

        let mut d_idx = 0;
        let mut c_idx = 0;

        while d_idx < d_rem.len() && c_idx < c_rem.len() {
            let d_amt = d_rem[d_idx].2;
            let c_amt = c_rem[c_idx].2;
            let pair_amt = d_amt.min(c_amt);

            if pair_amt > 0 {
                entries.push(JournalEntry {
                    id: Uuid::new_v4().to_string(),
                    date: self.date,
                    debit: d_rem[d_idx].0.clone(),
                    debit_sub: d_rem[d_idx].1.clone(),
                    credit: c_rem[c_idx].0.clone(),
                    credit_sub: c_rem[c_idx].1.clone(),
                    amount: pair_amt,
                    description: self.description.clone(),
                    created_at: self.created_at,
                });
            }

            d_rem[d_idx].2 -= pair_amt;
            c_rem[c_idx].2 -= pair_amt;

            if d_rem[d_idx].2 == 0 {
                d_idx += 1;
            }
            if c_rem[c_idx].2 == 0 {
                c_idx += 1;
            }
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_compound_transaction_fee_deducted() {
        // 売上10万円、振込手数料880円引かれて普通預金99,120円入金 (2借方 : 1貸方)
        let splits = vec![
            CompoundSplit::debit(AccountCode::Bank, Some("三井住友".into()), 99_120, None),
            CompoundSplit::debit(
                AccountCode::Miscellaneous,
                None,
                880,
                Some("振込手数料".into()),
            ),
            CompoundSplit::credit(AccountCode::Sales, None, 100_000, None),
        ];

        let tx = CompoundTransaction::new(
            NaiveDate::from_ymd_opt(2025, 5, 20).unwrap(),
            "売上入金（手数料差引）",
            splits,
        )
        .unwrap();

        assert_eq!(tx.total_amount(), 100_000);
        assert!(!tx.is_simple());

        let projected = tx.to_journal_entries();
        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].amount, 99_120);
        assert_eq!(projected[0].debit, AccountCode::Bank);
        assert_eq!(projected[0].credit, AccountCode::Sales);
        assert_eq!(projected[1].amount, 880);
        assert_eq!(projected[1].debit, AccountCode::Miscellaneous);
        assert_eq!(projected[1].credit, AccountCode::Sales);
    }

    #[test]
    fn test_unbalanced_splits_rejected() {
        let splits = vec![
            CompoundSplit::debit(AccountCode::Bank, None, 10_000, None),
            CompoundSplit::credit(AccountCode::Sales, None, 9_000, None),
        ];

        let err = CompoundTransaction::new(
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            "アンバランス",
            splits,
        )
        .unwrap_err();

        match err {
            CompoundError::UnbalancedSplits {
                debit_sum,
                credit_sum,
                diff,
            } => {
                assert_eq!(debit_sum, 10_000);
                assert_eq!(credit_sum, 9_000);
                assert_eq!(diff, 1_000);
            }
            _ => panic!("Expected UnbalancedSplits"),
        }
    }

    #[test]
    fn test_journal_entry_roundtrip() {
        let entry = JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
            AccountCode::Supplies,
            AccountCode::Cash,
            12_000,
            "消耗品購入",
        );

        let compound = CompoundTransaction::from_journal_entry(&entry);
        assert!(compound.is_simple());
        assert_eq!(compound.total_amount(), 12_000);

        let entries = compound.to_journal_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].amount, 12_000);
        assert_eq!(entries[0].debit, AccountCode::Supplies);
        assert_eq!(entries[0].credit, AccountCode::Cash);
    }
}
