//! Consumption Tax & Qualified Invoice System (消費税及びインボイス制度計算エンジン).
//!
//! Provides statutory consumption tax calculations for Japanese sole proprietors:
//! 1. Invoice registration (適格請求書発行事業者登録番号 T+13桁) validation & management
//! 2. Tax categories (10% standard, 8% reduced, Tax-exempt, Non-taxable) & auto-inference
//! 3. General taxation (本則課税 / 一般課税)
//! 4. Simplified taxation (簡易課税制度 第1種〜第6種 90%〜40%)
//! 5. Small-business 20% transition rule (2割特例)
//! 6. Statutory national (国税 7.8%/6.24%) and local (地方消費税 22/78) tax apportionment and truncation (100円未満切捨て)
//! 7. Multi-method comparison simulator (本則 vs 簡易 vs 2割特例の納税額比較)

use crate::core::journal::{AccountCode, AccountNature, LedgerDocument};
use serde::{Deserialize, Serialize};

/// 消費税区分
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaxCategory {
    /// 課税売上 / 課税仕入（標準税率 10%）
    #[default]
    Taxable10,
    /// 課税売上 / 課税仕入（軽減税率 8% - 飲食料品、新聞等）
    Taxable8Reduced,
    /// 非課税（受取利息、住宅家賃、土地賃貸、国債等）
    TaxExempt,
    /// 不課税 / 対象外（給与手当、福利厚生費の一部、租税公課、事業主勘定、元入金等）
    NonTaxable,
}

impl TaxCategory {
    pub fn label_ja(&self) -> &'static str {
        match self {
            Self::Taxable10 => "課税 10% (標準)",
            Self::Taxable8Reduced => "課税 8% (軽減)",
            Self::TaxExempt => "非課税",
            Self::NonTaxable => "対象外 (不課税)",
        }
    }

    pub fn rate_percent(&self) -> u32 {
        match self {
            Self::Taxable10 => 10,
            Self::Taxable8Reduced => 8,
            Self::TaxExempt | Self::NonTaxable => 0,
        }
    }

    /// 国税実効税率（10%のうち7.8%、8%のうち6.24%）
    pub fn national_rate(&self) -> f64 {
        match self {
            Self::Taxable10 => 0.078,
            Self::Taxable8Reduced => 0.0624,
            Self::TaxExempt | Self::NonTaxable => 0.0,
        }
    }

    /// 地方消費税率（国税の22/78。10%のうち2.2%、8%のうち1.76%）
    pub fn local_rate(&self) -> f64 {
        match self {
            Self::Taxable10 => 0.022,
            Self::Taxable8Reduced => 0.0176,
            Self::TaxExempt | Self::NonTaxable => 0.0,
        }
    }

    pub fn is_taxable(&self) -> bool {
        matches!(self, Self::Taxable10 | Self::Taxable8Reduced)
    }
}

/// 簡易課税の事業区分（第1種〜第6種）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SimplifiedBusinessCategory {
    /// 第1種: 卸売業（みなし仕入率 90%）
    Type1Wholesale,
    /// 第2種: 小売業、農業・林業・漁業（食料品）（みなし仕入率 80%）
    Type2Retail,
    /// 第3種: 製造業、建設業、情報通信業（ソフトウェア制作受託等）（みなし仕入率 70%）
    Type3Manufacturing,
    /// 第4種: 飲食店業、その他の事業（みなし仕入率 60%）
    Type4Other,
    /// 第5種: サービス業、ITサービス、コンサルティング、運輸業（みなし仕入率 50%）
    #[default]
    Type5Service,
    /// 第6種: 不動産業（みなし仕入率 40%）
    Type6RealEstate,
}

impl SimplifiedBusinessCategory {
    pub fn label_ja(&self) -> &'static str {
        match self {
            Self::Type1Wholesale => "第1種: 卸売業 (90%)",
            Self::Type2Retail => "第2種: 小売業 (80%)",
            Self::Type3Manufacturing => "第3種: 製造・建設・IT制作受託 (70%)",
            Self::Type4Other => "第4種: 飲食店業 (60%)",
            Self::Type5Service => "第5種: サービス業・コンサル (50%)",
            Self::Type6RealEstate => "第6種: 不動産業 (40%)",
        }
    }

    pub fn deemed_purchase_ratio(&self) -> f64 {
        match self {
            Self::Type1Wholesale => 0.90,
            Self::Type2Retail => 0.80,
            Self::Type3Manufacturing => 0.70,
            Self::Type4Other => 0.60,
            Self::Type5Service => 0.50,
            Self::Type6RealEstate => 0.40,
        }
    }

    pub fn ratio_percent(&self) -> u32 {
        match self {
            Self::Type1Wholesale => 90,
            Self::Type2Retail => 80,
            Self::Type3Manufacturing => 70,
            Self::Type4Other => 60,
            Self::Type5Service => 50,
            Self::Type6RealEstate => 40,
        }
    }

    pub fn all() -> &'static [SimplifiedBusinessCategory] {
        &[
            Self::Type1Wholesale,
            Self::Type2Retail,
            Self::Type3Manufacturing,
            Self::Type4Other,
            Self::Type5Service,
            Self::Type6RealEstate,
        ]
    }
}

/// 消費税計算方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaxCalcMethod {
    /// 本則課税（一般課税: 課税売上税額 - 課税仕入税額）
    General,
    /// 簡易課税（課税売上税額 × (1 - みなし仕入率)）
    Simplified,
    /// 2割特例（小規模事業者インボイス転換特例: 課税売上税額 × 20%）
    #[default]
    TwoTenthsSpecial,
}

impl TaxCalcMethod {
    pub fn label_ja(&self) -> &'static str {
        match self {
            Self::General => "本則課税 (一般課税)",
            Self::Simplified => "簡易課税制度",
            Self::TwoTenthsSpecial => "インボイス2割特例 (売上税額の20%のみ納付)",
        }
    }
}

/// 消費税・インボイス設定
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumptionTaxConfig {
    /// 課税事業者フラグ（false: 免税事業者）
    #[serde(default)]
    pub is_taxable_business: bool,
    /// 適格請求書発行事業者登録番号（T + 13桁数字、例: "T1234567890123"）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_registration_number: Option<String>,
    /// 採用する計算方式
    #[serde(default)]
    pub tax_calc_method: TaxCalcMethod,
    /// 簡易課税の事業区分
    #[serde(default)]
    pub simplified_category: SimplifiedBusinessCategory,
}

impl Default for ConsumptionTaxConfig {
    fn default() -> Self {
        Self {
            is_taxable_business: false,
            invoice_registration_number: None,
            tax_calc_method: TaxCalcMethod::TwoTenthsSpecial,
            simplified_category: SimplifiedBusinessCategory::Type5Service,
        }
    }
}

impl ConsumptionTaxConfig {
    /// 適格請求書発行事業者番号の形式チェック (T + 13桁)
    pub fn is_valid_invoice_number(num: &str) -> bool {
        let clean = num.trim();
        if clean.len() != 14 {
            return false;
        }
        let (prefix, digits) = clean.split_at(1);
        (prefix.eq_ignore_ascii_case("T")) && digits.chars().all(|c| c.is_ascii_digit())
    }
}

/// 勘定科目および摘要文から標準税区分を自動推論
pub fn infer_tax_category(account: &AccountCode, description: &str) -> TaxCategory {
    // 摘要に軽減税率キーワードが含まれる場合は 8%
    let desc_lower = description.to_lowercase();
    if desc_lower.contains("飲食料品")
        || desc_lower.contains("軽減税率")
        || desc_lower.contains("食品")
        || desc_lower.contains("テイクアウト")
        || desc_lower.contains("新聞")
    {
        return TaxCategory::Taxable8Reduced;
    }

    match account {
        // 売上高
        AccountCode::Sales => TaxCategory::Taxable10,
        // 売上原価・仕入高
        AccountCode::CostOfSales => TaxCategory::Taxable10,
        // 一般事業経費（原則 10% 課税仕入）
        AccountCode::Supplies
        | AccountCode::Communication
        | AccountCode::Advertising
        | AccountCode::Utilities
        | AccountCode::Repair
        | AccountCode::Travel
        | AccountCode::Entertainment
        | AccountCode::PackingShipping
        | AccountCode::Subcontracting
        | AccountCode::Miscellaneous => TaxCategory::Taxable10,

        // 地代家賃（事務所・店舗利用は課税10%）
        AccountCode::Rent => TaxCategory::Taxable10,

        // 固定資産購入
        AccountCode::ToolsAndEquipment
        | AccountCode::Vehicles
        | AccountCode::Buildings
        | AccountCode::Software => TaxCategory::Taxable10,

        // 減価償却費・福利厚生・租税公課・保険料・給料賃金・支払利息（非課税または不課税）
        AccountCode::Depreciation
        | AccountCode::Salaries
        | AccountCode::Welfare
        | AccountCode::TaxesAndDues
        | AccountCode::Insurance
        | AccountCode::Interest => TaxCategory::NonTaxable,

        // 資本・事業主勘定・現預金・債権債務
        AccountCode::Capital
        | AccountCode::Drawings
        | AccountCode::OwnerContribution
        | AccountCode::Cash
        | AccountCode::Bank
        | AccountCode::AccountsReceivable
        | AccountCode::AccountsPayable => TaxCategory::NonTaxable,

        // カスタム科目はデフォルトで課税10%
        AccountCode::Custom(_) => TaxCategory::Taxable10,
    }
}

/// 課税売上集計内訳
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TaxableSalesBreakdown {
    /// 10%標準課税売上高（税込）
    pub sales_10_inclusive: i64,
    /// 10%標準税抜売上高
    pub sales_10_exclusive: i64,
    /// 10%標準国税分（7.8%相当: 税込 × 7.8 / 110）
    pub tax_10_national: i64,
    /// 10%標準地方税分（2.2%相当: 税込 × 2.2 / 110）
    pub tax_10_local: i64,

    /// 8%軽減課税売上高（税込）
    pub sales_8_inclusive: i64,
    /// 8%軽減税抜売上高
    pub sales_8_exclusive: i64,
    /// 8%軽減国税分（6.24%相当: 税込 × 6.24 / 108）
    pub tax_8_national: i64,
    /// 8%軽減地方税分（1.76%相当: 税込 × 1.76 / 108）
    pub tax_8_local: i64,

    /// 非課税売上高（税込）
    pub exempt_sales: i64,
    /// 不課税・対象外売上高
    pub non_taxable_sales: i64,

    /// 課税売上高合計（税込: 10% + 8%）
    pub total_taxable_sales_inclusive: i64,
    /// 課税標準額（1,000円未満切捨て前の国税対象額）
    pub total_sales_tax_national: i64,
    /// 地方消費税合計
    pub total_sales_tax_local: i64,
}

/// 課税仕入集計内訳
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TaxablePurchaseBreakdown {
    /// 10%標準課税仕入高（税込）
    pub purchases_10_inclusive: i64,
    /// 10%標準税抜仕入高
    pub purchases_10_exclusive: i64,
    /// 10%標準国税分仕入税額
    pub tax_10_national: i64,

    /// 8%軽減課税仕入高（税込）
    pub purchases_8_inclusive: i64,
    /// 8%軽減税抜仕入高
    pub purchases_8_exclusive: i64,
    /// 8%軽減国税分仕入税額
    pub tax_8_national: i64,

    /// 課税仕入高合計（税込）
    pub total_taxable_purchases_inclusive: i64,
    /// 控除可能仕入国税額合計（本則課税時）
    pub total_purchase_tax_national: i64,
}

/// 方式別納税額シミュレーション比較
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TaxMethodSimulation {
    /// 本則課税での納付税額
    pub general_tax_payable: i64,
    /// 簡易課税での納付税額
    pub simplified_tax_payable: i64,
    /// 2割特例での納付税額
    pub two_tenths_tax_payable: i64,
    /// 最も有利（納税額が最小）な方式
    pub most_advantageous_method: TaxCalcMethod,
    /// 最小納税額
    pub min_tax_payable: i64,
}

/// 消費税計算の総合結果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumptionTaxSummary {
    /// 会計年度
    pub fiscal_year: i32,
    /// 設定
    pub config: ConsumptionTaxConfig,
    /// 課税売上内訳
    pub sales: TaxableSalesBreakdown,
    /// 課税仕入内訳
    pub purchases: TaxablePurchaseBreakdown,

    /// 課税標準額（国税: 課税売上高を1,000円未満切捨て）
    pub tax_base_amount: i64,
    /// 課税標準額に対する消費税額（国税分）
    pub output_tax_national: i64,
    /// 控除対象仕入税額（国税分）
    pub input_tax_deduction_national: i64,
    /// 差引国税納付税額（100円未満切捨て前）
    pub net_national_tax_before_truncation: i64,
    /// 差引国税納付税額（100円未満切捨て法定確定額）
    pub final_national_tax: i64,
    /// 地方消費税納付税額（final_national_tax × 22 / 78、100円未満切捨て法定確定額）
    pub final_local_tax: i64,
    /// 最終合計納付税額（国税 + 地方税）
    pub total_tax_payable: i64,

    /// 方式別比較シミュレーション
    pub simulation: TaxMethodSimulation,
}

/// 仕訳帳データから消費税・インボイスの完全計算を実行
pub fn calculate_consumption_tax(doc: &LedgerDocument) -> ConsumptionTaxSummary {
    let mut sales_10_inc = 0i64;
    let mut sales_8_inc = 0i64;
    let mut exempt_sales = 0i64;
    let mut non_tax_sales = 0i64;

    let mut purchases_10_inc = 0i64;
    let mut purchases_8_inc = 0i64;

    for entry in &doc.entries {
        // 売上取引の判定（貸方がSalesまたは収益科目）
        if entry.credit == AccountCode::Sales || entry.credit.nature() == AccountNature::Income {
            let cat = infer_tax_category(&entry.credit, &entry.description);
            match cat {
                TaxCategory::Taxable10 => sales_10_inc += entry.amount,
                TaxCategory::Taxable8Reduced => sales_8_inc += entry.amount,
                TaxCategory::TaxExempt => exempt_sales += entry.amount,
                TaxCategory::NonTaxable => non_tax_sales += entry.amount,
            }
        }

        // 経費・仕入取引の判定（借方がCostOfSalesまたは費用科目、資産科目）
        if entry.debit == AccountCode::CostOfSales
            || entry.debit.nature() == AccountNature::Expense
            || entry.debit.nature() == AccountNature::Asset
        {
            // 現金・預金移動・債権債務等は除外
            if matches!(
                entry.debit,
                AccountCode::Cash
                    | AccountCode::Bank
                    | AccountCode::AccountsReceivable
                    | AccountCode::AccountsPayable
                    | AccountCode::Drawings
            ) {
                continue;
            }
            let cat = infer_tax_category(&entry.debit, &entry.description);
            match cat {
                TaxCategory::Taxable10 => purchases_10_inc += entry.amount,
                TaxCategory::Taxable8Reduced => purchases_8_inc += entry.amount,
                TaxCategory::TaxExempt | TaxCategory::NonTaxable => {}
            }
        }
    }

    // 10% 国税分: 7.8 / 110, 地方税分: 2.2 / 110
    let tax_10_national = (sales_10_inc as f64 * (7.8 / 110.0)).round() as i64;
    let tax_10_local = (sales_10_inc as f64 * (2.2 / 110.0)).round() as i64;
    let sales_10_exc = sales_10_inc - tax_10_national - tax_10_local;

    // 8% 国税分: 6.24 / 108, 地方税分: 1.76 / 108
    let tax_8_national = (sales_8_inc as f64 * (6.24 / 108.0)).round() as i64;
    let tax_8_local = (sales_8_inc as f64 * (1.76 / 108.0)).round() as i64;
    let sales_8_exc = sales_8_inc - tax_8_national - tax_8_local;

    let total_sales_inc = sales_10_inc + sales_8_inc;
    let total_sales_tax_nat = tax_10_national + tax_8_national;
    let total_sales_tax_loc = tax_10_local + tax_8_local;

    let sales_breakdown = TaxableSalesBreakdown {
        sales_10_inclusive: sales_10_inc,
        sales_10_exclusive: sales_10_exc,
        tax_10_national,
        tax_10_local,
        sales_8_inclusive: sales_8_inc,
        sales_8_exclusive: sales_8_exc,
        tax_8_national,
        tax_8_local,
        exempt_sales,
        non_taxable_sales: non_tax_sales,
        total_taxable_sales_inclusive: total_sales_inc,
        total_sales_tax_national: total_sales_tax_nat,
        total_sales_tax_local: total_sales_tax_loc,
    };

    // 仕入税額計算
    let pur_tax_10_nat = (purchases_10_inc as f64 * (7.8 / 110.0)).round() as i64;
    let pur_tax_10_exc = purchases_10_inc - (purchases_10_inc as f64 * (10.0 / 110.0)).round() as i64;

    let pur_tax_8_nat = (purchases_8_inc as f64 * (6.24 / 108.0)).round() as i64;
    let pur_tax_8_exc = purchases_8_inc - (purchases_8_inc as f64 * (8.0 / 108.0)).round() as i64;

    let total_pur_inc = purchases_10_inc + purchases_8_inc;
    let total_pur_tax_nat = pur_tax_10_nat + pur_tax_8_nat;

    let purchase_breakdown = TaxablePurchaseBreakdown {
        purchases_10_inclusive: purchases_10_inc,
        purchases_10_exclusive: pur_tax_10_exc,
        tax_10_national: pur_tax_10_nat,
        purchases_8_inclusive: purchases_8_inc,
        purchases_8_exclusive: pur_tax_8_exc,
        tax_8_national: pur_tax_8_nat,
        total_taxable_purchases_inclusive: total_pur_inc,
        total_purchase_tax_national: total_pur_tax_nat,
    };

    // 課税標準額（1,000円未満切捨て）
    let tax_base_amount = (total_sales_inc / 1000) * 1000;
    let output_tax_national = total_sales_tax_nat;

    // 方式別税額の計算ヘルパー
    let calc_payable_for_method = |method: TaxCalcMethod, cat: SimplifiedBusinessCategory| -> i64 {
        let input_deduction = match method {
            TaxCalcMethod::General => total_pur_tax_nat,
            TaxCalcMethod::Simplified => {
                let ratio = cat.deemed_purchase_ratio();
                (output_tax_national as f64 * ratio).round() as i64
            }
            TaxCalcMethod::TwoTenthsSpecial => {
                // 2割特例: 売上税額の80%を控除（＝20%を納付）
                (output_tax_national as f64 * 0.80).round() as i64
            }
        };
        let net_nat = (output_tax_national - input_deduction).max(0);
        let final_nat = (net_nat / 100) * 100;
        let local = ((final_nat as f64 * (22.0 / 78.0)).floor() as i64 / 100) * 100;
        final_nat + local
    };

    let general_tax = calc_payable_for_method(TaxCalcMethod::General, doc.consumption_tax.simplified_category);
    let simplified_tax = calc_payable_for_method(TaxCalcMethod::Simplified, doc.consumption_tax.simplified_category);
    let two_tenths_tax = calc_payable_for_method(TaxCalcMethod::TwoTenthsSpecial, doc.consumption_tax.simplified_category);

    let min_tax = general_tax.min(simplified_tax).min(two_tenths_tax);
    let best_method = if min_tax == two_tenths_tax {
        TaxCalcMethod::TwoTenthsSpecial
    } else if min_tax == simplified_tax {
        TaxCalcMethod::Simplified
    } else {
        TaxCalcMethod::General
    };

    let simulation = TaxMethodSimulation {
        general_tax_payable: general_tax,
        simplified_tax_payable: simplified_tax,
        two_tenths_tax_payable: two_tenths_tax,
        most_advantageous_method: best_method,
        min_tax_payable: min_tax,
    };

    // 現在選択されている方式での仕入税額控除
    let input_tax_deduction_national = match doc.consumption_tax.tax_calc_method {
        TaxCalcMethod::General => total_pur_tax_nat,
        TaxCalcMethod::Simplified => {
            let ratio = doc.consumption_tax.simplified_category.deemed_purchase_ratio();
            (output_tax_national as f64 * ratio).round() as i64
        }
        TaxCalcMethod::TwoTenthsSpecial => {
            (output_tax_national as f64 * 0.80).round() as i64
        }
    };

    let net_national_tax_before_truncation = (output_tax_national - input_tax_deduction_national).max(0);
    // 差引国税額（100円未満切捨て）
    let final_national_tax = (net_national_tax_before_truncation / 100) * 100;
    // 地方消費税額（国税額 × 22 / 78、100円未満切捨て）
    let final_local_tax = ((final_national_tax as f64 * (22.0 / 78.0)).floor() as i64 / 100) * 100;
    let total_tax_payable = final_national_tax + final_local_tax;

    ConsumptionTaxSummary {
        fiscal_year: doc.fiscal_year,
        config: doc.consumption_tax.clone(),
        sales: sales_breakdown,
        purchases: purchase_breakdown,
        tax_base_amount,
        output_tax_national,
        input_tax_deduction_national,
        net_national_tax_before_truncation,
        final_national_tax,
        final_local_tax,
        total_tax_payable,
        simulation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::journal::JournalEntry;
    use chrono::NaiveDate;

    #[test]
    fn test_invoice_registration_number_validation() {
        assert!(ConsumptionTaxConfig::is_valid_invoice_number("T1234567890123"));
        assert!(ConsumptionTaxConfig::is_valid_invoice_number("t9876543210987"));
        assert!(ConsumptionTaxConfig::is_valid_invoice_number("  T1234567890123  "));

        // 不正形式
        assert!(!ConsumptionTaxConfig::is_valid_invoice_number("1234567890123")); // Tなし
        assert!(!ConsumptionTaxConfig::is_valid_invoice_number("T123456789012")); // 12桁
        assert!(!ConsumptionTaxConfig::is_valid_invoice_number("T12345678901234")); // 14桁
        assert!(!ConsumptionTaxConfig::is_valid_invoice_number("A1234567890123")); // A始まり
        assert!(!ConsumptionTaxConfig::is_valid_invoice_number(""));
    }

    #[test]
    fn test_tax_category_inference() {
        assert_eq!(
            infer_tax_category(&AccountCode::Sales, "コンサルティング報酬"),
            TaxCategory::Taxable10
        );
        assert_eq!(
            infer_tax_category(&AccountCode::Sales, "お弁当テイクアウト販売（飲食料品）"),
            TaxCategory::Taxable8Reduced
        );
        assert_eq!(
            infer_tax_category(&AccountCode::Supplies, "コピー用紙"),
            TaxCategory::Taxable10
        );
        assert_eq!(
            infer_tax_category(&AccountCode::Salaries, "給与支払"),
            TaxCategory::NonTaxable
        );
        assert_eq!(
            infer_tax_category(&AccountCode::TaxesAndDues, "固定資産税"),
            TaxCategory::NonTaxable
        );
    }

    #[test]
    fn test_two_tenths_special_and_simplified_calculation() {
        let mut doc = LedgerDocument::new(2026);
        doc.consumption_tax.is_taxable_business = true;
        doc.consumption_tax.tax_calc_method = TaxCalcMethod::TwoTenthsSpecial;
        doc.consumption_tax.simplified_category = SimplifiedBusinessCategory::Type5Service; // 50%

        let d = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        // 売上: 11,000,000 円 (税込10% -> 税抜10,000,000円, 国税分780,000円, 地方税分220,000円)
        doc.entries.push(JournalEntry::new(
            d,
            AccountCode::Bank,
            AccountCode::Sales,
            11_000_000,
            "年間売上高",
        ));

        // 経費: 3,300,000 円 (税込10% -> 国税分234,000円)
        doc.entries.push(JournalEntry::new(
            d,
            AccountCode::Supplies,
            AccountCode::Bank,
            3_300_000,
            "消耗品・外注費",
        ));

        let summary = calculate_consumption_tax(&doc);
        assert_eq!(summary.sales.sales_10_inclusive, 11_000_000);
        assert_eq!(summary.sales.tax_10_national, 780_000);
        assert_eq!(summary.sales.tax_10_local, 220_000);

        // 2割特例: 売上税額の20%のみ納付
        // 国税 = 780,000 * 0.20 = 156,000 円
        // 地方税 = 156,000 * 22 / 78 = 44,000 円
        // 合計 = 200,000 円
        assert_eq!(summary.final_national_tax, 156_000);
        assert_eq!(summary.final_local_tax, 44_000);
        assert_eq!(summary.total_tax_payable, 200_000);

        // シミュレーション比較
        // 本則課税: 国税 780,000 - 234,000 = 546,000円, 地方税 154,000円 -> 合計 700,000円
        assert_eq!(summary.simulation.general_tax_payable, 700_000);
        // 簡易課税 (第5種 50%): 国税 780,000 * 0.50 = 390,000円, 地方税 110,000円 -> 合計 500,000円
        assert_eq!(summary.simulation.simplified_tax_payable, 500_000);
        // 2割特例: 200,000円
        assert_eq!(summary.simulation.two_tenths_tax_payable, 200_000);
        assert_eq!(summary.simulation.most_advantageous_method, TaxCalcMethod::TwoTenthsSpecial);
        assert_eq!(summary.simulation.min_tax_payable, 200_000);
    }
}
