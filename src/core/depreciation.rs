//! Fixed Assets Ledger & Depreciation Engine (固定資産台帳・減価償却エンジン).
//!
//! Pure accounting logic for:
//! 1. Straight-Line method (定額法 - 平成19年4月1日以後取得)
//! 2. 3-Year Equal Lump-Sum Asset (一括償却資産: 取得価額20万円未満を3年間で均等償却)
//! 3. Immediate Depreciation for Small-Sum Assets (少額減価償却資産特例: 青色申告者30万円未満即時全額償却)
//! 4. Automatic generation and safe cleanup of year-end depreciation journal entries.

use crate::core::journal::{AccountCode, JournalEntry, LedgerDocument};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 減価償却方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DepreciationMethod {
    /// 定額法（個人事業主の法定償却方法）
    #[default]
    StraightLine,
    /// 一括償却資産（取得価額20万円未満、3年均等償却・月割なし・備忘価額なし）
    LumpSum3Year,
    /// 少額減価償却資産の特例（青色申告限定、取得価額30万円未満、取得年全額償却）
    ImmediateSmallAsset,
}

impl DepreciationMethod {
    pub fn label_ja(self) -> &'static str {
        match self {
            Self::StraightLine => "定額法",
            Self::LumpSum3Year => "一括償却（3年均等）",
            Self::ImmediateSmallAsset => "少額減価償却資産特例（即時償却）",
        }
    }

    pub fn all() -> &'static [DepreciationMethod] {
        &[
            Self::StraightLine,
            Self::LumpSum3Year,
            Self::ImmediateSmallAsset,
        ]
    }
}

/// 固定資産台帳の1レコード
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedAsset {
    pub id: String,
    /// 資産の名称（例: "MacBook Pro M3", "社用車", "オフィス用デスク"）
    pub name: String,
    /// 取得年月日
    pub acquisition_date: NaiveDate,
    /// 取得価額（円）
    pub acquisition_cost: i64,
    /// 法定耐用年数（定額法で使用。一括償却・少額特例では参考値）
    pub useful_life_years: u32,
    /// 償却方法
    pub depreciation_method: DepreciationMethod,
    /// 事業専用割合（1〜100%）
    pub business_ratio_percent: u8,
    /// 資産に対応する勘定科目（デフォルト: 工具器具備品）
    #[serde(default = "default_asset_account")]
    pub asset_account: AccountCode,
    /// メモ・摘要
    #[serde(default)]
    pub memo: String,
}

fn default_asset_account() -> AccountCode {
    AccountCode::ToolsAndEquipment
}

impl FixedAsset {
    pub fn new(
        name: impl Into<String>,
        acquisition_date: NaiveDate,
        acquisition_cost: i64,
        useful_life_years: u32,
        depreciation_method: DepreciationMethod,
        business_ratio_percent: u8,
        asset_account: AccountCode,
        memo: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            acquisition_date,
            acquisition_cost,
            useful_life_years,
            depreciation_method,
            business_ratio_percent: business_ratio_percent.clamp(1, 100),
            asset_account,
            memo: memo.into(),
        }
    }
}

/// 定額法償却率表（平成19年4月1日以後取得分）
/// 国税庁の減価償却資産の償却率表に基づく
pub fn straight_line_depreciation_rate(useful_life: u32) -> f64 {
    match useful_life {
        2 => 0.500,
        3 => 0.334,
        4 => 0.250,
        5 => 0.200,
        6 => 0.167,
        7 => 0.143,
        8 => 0.125,
        9 => 0.112,
        10 => 0.100,
        11 => 0.091,
        12 => 0.084,
        13 => 0.077,
        14 => 0.072,
        15 => 0.067,
        16 => 0.063,
        17 => 0.059,
        18 => 0.056,
        19 => 0.053,
        20 => 0.050,
        22 => 0.046,
        24 => 0.042,
        25 => 0.040,
        30 => 0.034,
        35 => 0.029,
        40 => 0.025,
        45 => 0.023,
        50 => 0.020,
        n if n > 0 => ((1.0 / n as f64) * 1000.0).ceil() / 1000.0,
        _ => 0.0,
    }
}

/// 単一資産の年度別減価償却計算結果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepreciationCalculation {
    pub asset_id: String,
    pub name: String,
    pub acquisition_date: NaiveDate,
    pub acquisition_cost: i64,
    pub useful_life_years: u32,
    pub depreciation_method: DepreciationMethod,
    pub asset_account: AccountCode,
    /// 期首帳簿価額（未償却残高）
    pub opening_book_value: i64,
    /// 適用償却率 (定額法の場合、整数パーセントまたは小数表示用)
    pub depreciation_rate_per_mille: u32, // 例: 250 = 0.250
    /// 本年中の使用月数 (取得年は 12 - 取得月 + 1, それ以降は 12)
    pub months_in_year: u32,
    /// 本年分の普通償却額（全額）
    pub ordinary_depreciation: i64,
    /// 期末帳簿価額（未償却残高）
    pub closing_book_value: i64,
    /// 事業専用割合 (%)
    pub business_ratio_percent: u8,
    /// 本年分の必要経費算入額 (普通償却額 * 事業割合 / 100)
    pub business_expense: i64,
    /// 家事按分分 (普通償却額 - 必要経費算入額)
    pub private_portion: i64,
}

/// 指定年度における資産の減価償却を計算する
pub fn calculate_asset_depreciation(asset: &FixedAsset, fiscal_year: i32) -> DepreciationCalculation {
    let acq_year = asset.acquisition_date.year();

    // 取得年度より前の年度の場合は償却対象外
    if fiscal_year < acq_year {
        return DepreciationCalculation {
            asset_id: asset.id.clone(),
            name: asset.name.clone(),
            acquisition_date: asset.acquisition_date,
            acquisition_cost: asset.acquisition_cost,
            useful_life_years: asset.useful_life_years,
            depreciation_method: asset.depreciation_method,
            asset_account: asset.asset_account.clone(),
            opening_book_value: 0,
            depreciation_rate_per_mille: 0,
            months_in_year: 0,
            ordinary_depreciation: 0,
            closing_book_value: 0,
            business_ratio_percent: asset.business_ratio_percent,
            business_expense: 0,
            private_portion: 0,
        };
    }

    match asset.depreciation_method {
        DepreciationMethod::ImmediateSmallAsset => {
            if fiscal_year == acq_year {
                let ordinary = asset.acquisition_cost;
                let business_expense = (ordinary * asset.business_ratio_percent as i64) / 100;
                let private_portion = ordinary - business_expense;
                DepreciationCalculation {
                    asset_id: asset.id.clone(),
                    name: asset.name.clone(),
                    acquisition_date: asset.acquisition_date,
                    acquisition_cost: asset.acquisition_cost,
                    useful_life_years: asset.useful_life_years,
                    depreciation_method: asset.depreciation_method,
                    asset_account: asset.asset_account.clone(),
                    opening_book_value: asset.acquisition_cost,
                    depreciation_rate_per_mille: 1000,
                    months_in_year: 12 - asset.acquisition_date.month() + 1,
                    ordinary_depreciation: ordinary,
                    closing_book_value: 0,
                    business_ratio_percent: asset.business_ratio_percent,
                    business_expense,
                    private_portion,
                }
            } else {
                // 取得翌年以降は簿価0、償却費0
                DepreciationCalculation {
                    asset_id: asset.id.clone(),
                    name: asset.name.clone(),
                    acquisition_date: asset.acquisition_date,
                    acquisition_cost: asset.acquisition_cost,
                    useful_life_years: asset.useful_life_years,
                    depreciation_method: asset.depreciation_method,
                    asset_account: asset.asset_account.clone(),
                    opening_book_value: 0,
                    depreciation_rate_per_mille: 0,
                    months_in_year: 0,
                    ordinary_depreciation: 0,
                    closing_book_value: 0,
                    business_ratio_percent: asset.business_ratio_percent,
                    business_expense: 0,
                    private_portion: 0,
                }
            }
        }
        DepreciationMethod::LumpSum3Year => {
            // 一括償却: 3年均等償却、月割なし、備忘価額1円は残さず0円まで償却
            let year_index = fiscal_year - acq_year; // 0, 1, 2
            let y1_dep = asset.acquisition_cost / 3;
            let y2_dep = asset.acquisition_cost / 3;
            let y3_dep = asset.acquisition_cost - y1_dep - y2_dep;

            let (opening, ordinary, closing) = match year_index {
                0 => (asset.acquisition_cost, y1_dep, asset.acquisition_cost - y1_dep),
                1 => (
                    asset.acquisition_cost - y1_dep,
                    y2_dep,
                    asset.acquisition_cost - y1_dep - y2_dep,
                ),
                2 => (y3_dep, y3_dep, 0),
                _ => (0, 0, 0),
            };

            let business_expense = (ordinary * asset.business_ratio_percent as i64) / 100;
            let private_portion = ordinary - business_expense;

            DepreciationCalculation {
                asset_id: asset.id.clone(),
                name: asset.name.clone(),
                acquisition_date: asset.acquisition_date,
                acquisition_cost: asset.acquisition_cost,
                useful_life_years: 3,
                depreciation_method: asset.depreciation_method,
                asset_account: asset.asset_account.clone(),
                opening_book_value: opening,
                depreciation_rate_per_mille: 334,
                months_in_year: 12,
                ordinary_depreciation: ordinary,
                closing_book_value: closing,
                business_ratio_percent: asset.business_ratio_percent,
                business_expense,
                private_portion,
            }
        }
        DepreciationMethod::StraightLine => {
            let rate = straight_line_depreciation_rate(asset.useful_life_years);
            let rate_per_mille = (rate * 1000.0).round() as u32;

            // 取得年から目的年度まで期首帳簿価額と償却額をシミュレーション
            let mut current_book_value = asset.acquisition_cost;

            for y in acq_year..=fiscal_year {
                let months = if y == acq_year {
                    12 - asset.acquisition_date.month() + 1
                } else {
                    12
                };

                let opening = current_book_value;
                // 通常の年額償却限度額
                let std_dep = if y == acq_year {
                    ((asset.acquisition_cost as f64 * rate * months as f64) / 12.0).floor() as i64
                } else {
                    (asset.acquisition_cost as f64 * rate).floor() as i64
                };

                // 備忘価額1円を残すため、償却限度額は opening - 1
                let max_dep = (opening - 1).max(0);
                let ordinary = std_dep.min(max_dep);
                let closing = opening - ordinary;

                if y == fiscal_year {
                    let business_expense =
                        (ordinary * asset.business_ratio_percent as i64) / 100;
                    let private_portion = ordinary - business_expense;
                    return DepreciationCalculation {
                        asset_id: asset.id.clone(),
                        name: asset.name.clone(),
                        acquisition_date: asset.acquisition_date,
                        acquisition_cost: asset.acquisition_cost,
                        useful_life_years: asset.useful_life_years,
                        depreciation_method: asset.depreciation_method,
                        asset_account: asset.asset_account.clone(),
                        opening_book_value: opening,
                        depreciation_rate_per_mille: rate_per_mille,
                        months_in_year: months,
                        ordinary_depreciation: ordinary,
                        closing_book_value: closing,
                        business_ratio_percent: asset.business_ratio_percent,
                        business_expense,
                        private_portion,
                    };
                }

                current_book_value = closing;
            }

            // Fallback unreachable
            DepreciationCalculation {
                asset_id: asset.id.clone(),
                name: asset.name.clone(),
                acquisition_date: asset.acquisition_date,
                acquisition_cost: asset.acquisition_cost,
                useful_life_years: asset.useful_life_years,
                depreciation_method: asset.depreciation_method,
                asset_account: asset.asset_account.clone(),
                opening_book_value: 0,
                depreciation_rate_per_mille: 0,
                months_in_year: 0,
                ordinary_depreciation: 0,
                closing_book_value: 0,
                business_ratio_percent: asset.business_ratio_percent,
                business_expense: 0,
                private_portion: 0,
            }
        }
    }
}

/// LedgerDocument 内の全固定資産の指定年度の減価償却一覧を計算
pub fn calculate_all_depreciations(doc: &LedgerDocument) -> Vec<DepreciationCalculation> {
    doc.fixed_assets
        .iter()
        .map(|asset| calculate_asset_depreciation(asset, doc.fiscal_year))
        .collect()
}

pub const DEPRECIATION_ENTRY_PREFIX: &str = "[減価償却]";

/// 該当仕訳が減価償却によって自動生成されたものかを判定
pub fn is_depreciation_entry(entry: &JournalEntry) -> bool {
    entry.description.starts_with(DEPRECIATION_ENTRY_PREFIX)
        && (entry.debit == AccountCode::Depreciation || entry.debit == AccountCode::Drawings)
}

/// 減価償却仕訳の自動生成（12月31日付）
pub fn generate_depreciation_entries(doc: &LedgerDocument) -> Vec<JournalEntry> {
    let calcs = calculate_all_depreciations(doc);
    let date = match NaiveDate::from_ymd_opt(doc.fiscal_year, 12, 31) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();
    for c in calcs {
        if c.ordinary_depreciation <= 0 {
            continue;
        }

        // 1. 事業用必要経費分 (借方: 減価償却費 / 貸方: 資産勘定)
        if c.business_expense > 0 {
            let desc = format!(
                "{} {} ({})",
                DEPRECIATION_ENTRY_PREFIX,
                c.name,
                c.depreciation_method.label_ja()
            );
            entries.push(JournalEntry::new(
                date,
                AccountCode::Depreciation,
                c.asset_account.clone(),
                c.business_expense,
                desc,
            ));
        }

        // 2. 家事按分・事業主負担分 (借方: 事業主貸 / 貸方: 資産勘定)
        if c.private_portion > 0 {
            let desc = format!(
                "{}・事業主分 {} ({})",
                DEPRECIATION_ENTRY_PREFIX,
                c.name,
                c.depreciation_method.label_ja()
            );
            entries.push(JournalEntry::new(
                date,
                AccountCode::Drawings,
                c.asset_account.clone(),
                c.private_portion,
                desc,
            ));
        }
    }
    entries
}

/// 既存の減価償却仕訳を安全に削除して再生成・反映
pub fn apply_depreciation_entries(doc: &mut LedgerDocument) -> usize {
    clear_depreciation_entries(doc);
    let new_entries = generate_depreciation_entries(doc);
    let count = new_entries.len();
    doc.entries.extend(new_entries);
    count
}

/// 既存の自動生成減価償却仕訳を全削除
pub fn clear_depreciation_entries(doc: &mut LedgerDocument) -> usize {
    let before = doc.entries.len();
    doc.entries.retain(|e| !is_depreciation_entry(e));
    before - doc.entries.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_straight_line_depreciation_pc_4years() {
        // 4年耐用年数（償却率 0.250）
        // 取得価額: 200,000円、2025年4月1日取得（本年初年度9ヶ月）
        let asset = FixedAsset::new(
            "開発用ノートPC",
            NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
            200_000,
            4,
            DepreciationMethod::StraightLine,
            100,
            AccountCode::ToolsAndEquipment,
            "M3 Max MacBook",
        );

        // 2025年（初年度: 9ヶ月）
        // 200,000 * 0.250 * 9 / 12 = 37,500円
        let calc_2025 = calculate_asset_depreciation(&asset, 2025);
        assert_eq!(calc_2025.opening_book_value, 200_000);
        assert_eq!(calc_2025.months_in_year, 9);
        assert_eq!(calc_2025.ordinary_depreciation, 37_500);
        assert_eq!(calc_2025.closing_book_value, 162_500);
        assert_eq!(calc_2025.business_expense, 37_500);

        // 2026年（2年目: 12ヶ月）
        // 200,000 * 0.250 = 50,000円
        let calc_2026 = calculate_asset_depreciation(&asset, 2026);
        assert_eq!(calc_2026.opening_book_value, 162_500);
        assert_eq!(calc_2026.ordinary_depreciation, 50_000);
        assert_eq!(calc_2026.closing_book_value, 112_500);

        // 2027年（3年目: 12ヶ月）
        let calc_2027 = calculate_asset_depreciation(&asset, 2027);
        assert_eq!(calc_2027.opening_book_value, 112_500);
        assert_eq!(calc_2027.ordinary_depreciation, 50_000);
        assert_eq!(calc_2027.closing_book_value, 62_500);

        // 2028年（4年目: 12ヶ月）
        let calc_2028 = calculate_asset_depreciation(&asset, 2028);
        assert_eq!(calc_2028.opening_book_value, 62_500);
        assert_eq!(calc_2028.ordinary_depreciation, 50_000);
        assert_eq!(calc_2028.closing_book_value, 12_500);

        // 2029年（5年目: 残り12,500円のうち備忘価額1円を残して12,499円償却）
        let calc_2029 = calculate_asset_depreciation(&asset, 2029);
        assert_eq!(calc_2029.opening_book_value, 12_500);
        assert_eq!(calc_2029.ordinary_depreciation, 12_499);
        assert_eq!(calc_2029.closing_book_value, 1);

        // 2030年（6年目: 備忘価額1円のみ残存、償却額0円）
        let calc_2030 = calculate_asset_depreciation(&asset, 2030);
        assert_eq!(calc_2030.opening_book_value, 1);
        assert_eq!(calc_2030.ordinary_depreciation, 0);
        assert_eq!(calc_2030.closing_book_value, 1);
    }

    #[test]
    fn test_lump_sum_3year_depreciation() {
        // 一括償却資産: 取得価額 150,000円（3年均等 50,000円ずつ）
        let asset = FixedAsset::new(
            "業務用エアコン",
            NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(),
            150_000,
            3,
            DepreciationMethod::LumpSum3Year,
            100,
            AccountCode::ToolsAndEquipment,
            "一括償却テスト",
        );

        let c1 = calculate_asset_depreciation(&asset, 2025);
        assert_eq!(c1.opening_book_value, 150_000);
        assert_eq!(c1.ordinary_depreciation, 50_000);
        assert_eq!(c1.closing_book_value, 100_000);

        let c2 = calculate_asset_depreciation(&asset, 2026);
        assert_eq!(c2.opening_book_value, 100_000);
        assert_eq!(c2.ordinary_depreciation, 50_000);
        assert_eq!(c2.closing_book_value, 50_000);

        let c3 = calculate_asset_depreciation(&asset, 2027);
        assert_eq!(c3.opening_book_value, 50_000);
        assert_eq!(c3.ordinary_depreciation, 50_000);
        assert_eq!(c3.closing_book_value, 0); // 備忘価額なしで0円まで償却

        let c4 = calculate_asset_depreciation(&asset, 2028);
        assert_eq!(c4.ordinary_depreciation, 0);
        assert_eq!(c4.closing_book_value, 0);
    }

    #[test]
    fn test_immediate_small_asset_special() {
        // 少額特例（30万円未満、即時全額償却）
        let asset = FixedAsset::new(
            "高性能カメラ",
            NaiveDate::from_ymd_opt(2025, 5, 10).unwrap(),
            280_000,
            5,
            DepreciationMethod::ImmediateSmallAsset,
            100,
            AccountCode::ToolsAndEquipment,
            "少額特例",
        );

        let c1 = calculate_asset_depreciation(&asset, 2025);
        assert_eq!(c1.opening_book_value, 280_000);
        assert_eq!(c1.ordinary_depreciation, 280_000);
        assert_eq!(c1.closing_book_value, 0);

        let c2 = calculate_asset_depreciation(&asset, 2026);
        assert_eq!(c2.ordinary_depreciation, 0);
        assert_eq!(c2.closing_book_value, 0);
    }

    #[test]
    fn test_home_office_allocation_on_asset() {
        // 事業専用割合 60%
        let asset = FixedAsset::new(
            "兼用自動車",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            1_000_000,
            6, // 0.167
            DepreciationMethod::StraightLine,
            60,
            AccountCode::Vehicles,
            "車",
        );

        let c = calculate_asset_depreciation(&asset, 2025);
        // 普通償却額 = 1,000,000 * 0.167 = 167,000円
        assert_eq!(c.ordinary_depreciation, 167_000);
        // 必要経費 = 167,000 * 60% = 100,200円
        assert_eq!(c.business_expense, 100_200);
        // 事業主分 = 167,000 - 100,200 = 66,800円
        assert_eq!(c.private_portion, 66_800);
    }

    #[test]
    fn test_generate_and_apply_depreciation_entries() {
        let mut doc = LedgerDocument::new(2025);
        doc.fixed_assets.push(FixedAsset::new(
            "PC",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            200_000,
            4,
            DepreciationMethod::StraightLine,
            80, // 80% 事業
            AccountCode::ToolsAndEquipment,
            "",
        ));

        let count = apply_depreciation_entries(&mut doc);
        assert_eq!(count, 2); // 経費分 + 事業主分
        assert_eq!(doc.entries.len(), 2);

        let exp_entry = doc
            .entries
            .iter()
            .find(|e| e.debit == AccountCode::Depreciation)
            .unwrap();
        assert_eq!(exp_entry.credit, AccountCode::ToolsAndEquipment);
        assert_eq!(exp_entry.amount, 40_000); // 50,000 * 80%

        let priv_entry = doc
            .entries
            .iter()
            .find(|e| e.debit == AccountCode::Drawings)
            .unwrap();
        assert_eq!(priv_entry.credit, AccountCode::ToolsAndEquipment);
        assert_eq!(priv_entry.amount, 10_000); // 50,000 * 20%

        // 再実行しても重複せず更新されること（冪等性）
        let count2 = apply_depreciation_entries(&mut doc);
        assert_eq!(count2, 2);
        assert_eq!(doc.entries.len(), 2);

        // クリア
        clear_depreciation_entries(&mut doc);
        assert_eq!(doc.entries.len(), 0);
    }
}
