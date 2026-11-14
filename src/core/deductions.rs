//! Personal Income Tax Deductions (所得控除) & Tax Credits (税額控除) Engine.
//!
//! Pure tax logic for:
//! 1. Basic deduction (基礎控除: 高所得者逓減含む)
//! 2. Social insurance (社会保険料控除)
//! 3. Small enterprise mutual aid & iDeCo (小規模企業共済等掛金控除)
//! 4. Life insurance (生命保険料控除: 新・旧一般、介護医療、新・旧個人年金、限度額12万円)
//! 5. Earthquake insurance (地震保険料控除: 旧長期損害保険料併用、限度額5万円)
//! 6. Medical expenses (医療費控除: 足切り10万/総所得5%判定、上限200万円)
//! 7. Donations & Furusato Nozei (寄附金控除: 総所得40%限度、足切り2,000円)
//! 8. Spouse & Special Spouse deduction (配偶者控除・配偶者特別控除: 本人所得制限対応)
//! 9. Dependents deduction (扶養控除: 一般38万、特定63万、老人同居58万、老人別居48万)
//! 10. Disability, Widow/Single Parent, Working student (障害者、寡婦・ひとり親、勤労学生)
//! 11. Tax credits (住宅借入金等特別控除・税額控除)

use serde::{Deserialize, Serialize};

/// 配偶者控除の区分
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpouseDeductionType {
    /// 控除なし
    #[default]
    None,
    /// 配偶者控除（一般: 配偶者70歳未満）基準額38万円
    General,
    /// 配偶者控除（老人: 配偶者70歳以上）基準額48万円
    Elderly,
    /// 配偶者特別控除（配偶者所得48万超133万以下）
    Special {
        /// 配偶者特別控除額（満額38万円、配偶者所得に応じて逓減）
        amount: i64,
    },
}

impl SpouseDeductionType {
    pub fn label_ja(self) -> &'static str {
        match self {
            Self::None => "なし",
            Self::General => "配偶者控除（一般・38万円）",
            Self::Elderly => "配偶者控除（老人・48万円）",
            Self::Special { .. } => "配偶者特別控除",
        }
    }
}

/// 寡婦・ひとり親控除の区分
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WidowOrSingleParentType {
    #[default]
    None,
    /// 寡婦控除（控除額27万円、所得500万円以下）
    Widow,
    /// ひとり親控除（控除額35万円、所得500万円以下）
    SingleParent,
}

impl WidowOrSingleParentType {
    pub fn label_ja(self) -> &'static str {
        match self {
            Self::None => "なし",
            Self::Widow => "寡婦控除（27万円）",
            Self::SingleParent => "ひとり親控除（35万円）",
        }
    }
}

/// ユーザー入力用の各種所得控除・税額控除設定
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TaxDeductions {
    /// 社会保険料控除（国民健康保険・国民年金・介護保険等の支払合計額）
    #[serde(default)]
    pub social_insurance: i64,

    /// 小規模企業共済等掛金控除（小規模企業共済掛金、iDeCo確定拠出年金等の支払合計額）
    #[serde(default)]
    pub small_enterprise_mutual_aid: i64,

    // ---- 生命保険料控除 (支払保険料の額) ----
    /// 新・一般生命保険料（平成24年以後契約）支払額
    #[serde(default)]
    pub life_insurance_new_general: i64,
    /// 介護医療保険料（平成24年以後契約）支払額
    #[serde(default)]
    pub life_insurance_new_care: i64,
    /// 新・個人年金保険料（平成24年以後契約）支払額
    #[serde(default)]
    pub life_insurance_new_annuity: i64,
    /// 旧・一般生命保険料（平成23年以前契約）支払額
    #[serde(default)]
    pub life_insurance_old_general: i64,
    /// 旧・個人年金保険料（平成23年以前契約）支払額
    #[serde(default)]
    pub life_insurance_old_annuity: i64,

    // ---- 地震保険料控除 (支払保険料の額) ----
    /// 地震保険料 支払額
    #[serde(default)]
    pub earthquake_insurance: i64,
    /// 旧長期損害保険料 支払額
    #[serde(default)]
    pub earthquake_old_long_term: i64,

    // ---- 医療費控除 ----
    /// 支払った医療費の総額
    #[serde(default)]
    pub medical_expenses_paid: i64,
    /// 保険金等で補填される金額（高額療養費、生命保険入院給付金等）
    #[serde(default)]
    pub medical_expenses_reimbursed: i64,

    // ---- 寄附金控除（ふるさと納税等） ----
    /// 支払った寄附金の総額（ふるさと納税含む）
    #[serde(default)]
    pub donations_paid: i64,

    // ---- 配偶者控除・扶養控除 ----
    /// 配偶者控除区分
    #[serde(default)]
    pub spouse_type: SpouseDeductionType,
    /// 一般扶養親族の人数（16歳以上19歳未満、23歳以上70歳未満）1人あたり38万円
    #[serde(default)]
    pub dependents_general_count: u32,
    /// 特定扶養親族の人数（19歳以上23歳未満）1人あたり63万円
    #[serde(default)]
    pub dependents_specific_count: u32,
    /// 老人扶養親族（同居老親等）の人数（70歳以上）1人あたり58万円
    #[serde(default)]
    pub dependents_elderly_living_count: u32,
    /// 老人扶養親族（同居以外）の人数（70歳以上）1人あたり48万円
    #[serde(default)]
    pub dependents_elderly_other_count: u32,

    // ---- 障害者・ひとり親・勤労学生 ----
    /// 障害者控除合計額（普通27万、特別40万、同居特別75万等の合算額）
    #[serde(default)]
    pub disability_deduction: i64,
    /// 寡婦・ひとり親控除
    #[serde(default)]
    pub widow_or_single_parent: WidowOrSingleParentType,
    /// 勤労学生控除（27万円）
    #[serde(default)]
    pub working_student: bool,

    // ---- 税額控除 ----
    /// 住宅借入金等特別控除（住宅ローン控除）額
    #[serde(default)]
    pub housing_loan_tax_credit: i64,
    /// その他の税額控除（配当控除・政党等寄附金特別控除等）
    #[serde(default)]
    pub other_tax_credits: i64,
}

/// 計算後の所得控除および税額控除の明細
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CalculatedDeductions {
    pub basic: i64,
    pub social_insurance: i64,
    pub small_enterprise_mutual_aid: i64,
    pub life_insurance: i64,
    pub earthquake_insurance: i64,
    pub medical: i64,
    pub donation: i64,
    pub spouse: i64,
    pub dependents: i64,
    pub disability: i64,
    pub widow_or_single_parent: i64,
    pub working_student: i64,
    /// 全所得控除の総合計
    pub total_deductions: i64,

    // 税額控除
    pub housing_loan_tax_credit: i64,
    pub other_tax_credits: i64,
    /// 全税額控除の総合計
    pub total_tax_credits: i64,
}

/// 新制度の生命保険料控除計算（1枠あたり最高4万円）
pub fn compute_new_life_insurance_tier(premium: i64) -> i64 {
    if premium <= 0 {
        0
    } else if premium <= 20_000 {
        premium
    } else if premium <= 40_000 {
        (premium / 2) + 10_000
    } else if premium <= 80_000 {
        (premium / 4) + 20_000
    } else {
        40_000
    }
}

/// 旧制度の生命保険料控除計算（1枠あたり最高5万円）
pub fn compute_old_life_insurance_tier(premium: i64) -> i64 {
    if premium <= 0 {
        0
    } else if premium <= 25_000 {
        premium
    } else if premium <= 50_000 {
        (premium / 2) + 12_500
    } else if premium <= 100_000 {
        (premium / 4) + 25_000
    } else {
        50_000
    }
}

/// 新旧併用枠（一般または年金）の合算計算
/// 新制度と旧制度の双方について控除の適用を受ける場合:
/// 新契約の控除額 ＋ 旧契約の控除額（最高4万円）
/// または、旧契約の控除額のみ（最高5万円）のいずれか有利な方を選択可能
pub fn compute_combined_life_insurance(new_premium: i64, old_premium: i64) -> i64 {
    if old_premium <= 0 {
        return compute_new_life_insurance_tier(new_premium);
    }
    if new_premium <= 0 {
        return compute_old_life_insurance_tier(old_premium);
    }

    let new_ded = compute_new_life_insurance_tier(new_premium);
    let old_ded = compute_old_life_insurance_tier(old_premium);

    let combined = (new_ded + old_ded).min(40_000);
    combined.max(old_ded)
}

/// 生命保険料控除全体の合算（最高12万円）
pub fn compute_total_life_insurance_deduction(
    new_general: i64,
    old_general: i64,
    new_care: i64,
    new_annuity: i64,
    old_annuity: i64,
) -> i64 {
    let general = compute_combined_life_insurance(new_general, old_general);
    let care = compute_new_life_insurance_tier(new_care);
    let annuity = compute_combined_life_insurance(new_annuity, old_annuity);

    (general + care + annuity).min(120_000)
}

/// 地震保険料控除の計算（最高5万円）
pub fn compute_earthquake_insurance_deduction(earthquake: i64, old_long_term: i64) -> i64 {
    let eq_ded = earthquake.clamp(0, 50_000);

    let old_ded = if old_long_term <= 0 {
        0
    } else if old_long_term <= 10_000 {
        old_long_term
    } else if old_long_term <= 20_000 {
        (old_long_term / 2) + 5_000
    } else {
        15_000
    };

    (eq_ded + old_ded).min(50_000)
}

/// 医療費控除の計算（最高200万円）
/// 控除額 = (支払医療費 - 保険補填) - min(10万円, 総所得金額 * 5%)
pub fn compute_medical_deduction(paid: i64, reimbursed: i64, total_income: i64) -> i64 {
    let net_paid = (paid - reimbursed).max(0);
    if net_paid <= 0 {
        return 0;
    }
    let threshold = 100_000.min((total_income * 5) / 100).max(0);
    (net_paid - threshold).clamp(0, 2_000_000)
}

/// 寄附金控除（ふるさと納税等）の計算
/// 控除額 = min(寄附金合計, 総所得 * 40%) - 2,000円
pub fn compute_donation_deduction(paid: i64, total_income: i64) -> i64 {
    if paid <= 2_000 {
        return 0;
    }
    let income_limit = (total_income * 40) / 100;
    let eligible = paid.min(income_limit);
    (eligible - 2_000).max(0)
}

/// 配偶者控除・配偶者特別控除の計算
/// 本人の合計所得金額が900万円以下、950万円以下、1,000万円以下で段階的制限
pub fn compute_spouse_deduction(spouse: SpouseDeductionType, total_income: i64) -> i64 {
    let base_amount = match spouse {
        SpouseDeductionType::None => return 0,
        SpouseDeductionType::General => 380_000,
        SpouseDeductionType::Elderly => 480_000,
        SpouseDeductionType::Special { amount } => amount.clamp(0, 380_000),
    };

    if total_income <= 9_000_000 {
        base_amount
    } else if total_income <= 9_500_000 {
        // 約 2/3 (一般26万、老人32万)
        match spouse {
            SpouseDeductionType::General => 260_000,
            SpouseDeductionType::Elderly => 320_000,
            SpouseDeductionType::Special { amount } => (amount * 2) / 3,
            _ => 0,
        }
    } else if total_income <= 10_000_000 {
        // 約 1/3 (一般13万、老人16万)
        match spouse {
            SpouseDeductionType::General => 130_000,
            SpouseDeductionType::Elderly => 160_000,
            SpouseDeductionType::Special { amount } => amount / 3,
            _ => 0,
        }
    } else {
        0
    }
}

/// 扶養控除の計算
/// 一般: 38万/人、特定: 63万/人、老人同居: 58万/人、老人別居: 48万/人
pub fn compute_dependents_deduction(
    general: u32,
    specific: u32,
    elderly_living: u32,
    elderly_other: u32,
) -> i64 {
    (general as i64 * 380_000)
        + (specific as i64 * 630_000)
        + (elderly_living as i64 * 580_000)
        + (elderly_other as i64 * 480_000)
}

/// 寡婦・ひとり親控除の計算（合計所得500万円以下のみ適用）
pub fn compute_widow_or_single_parent_deduction(
    kind: WidowOrSingleParentType,
    total_income: i64,
) -> i64 {
    if total_income > 5_000_000 {
        return 0;
    }
    match kind {
        WidowOrSingleParentType::None => 0,
        WidowOrSingleParentType::Widow => 270_000,
        WidowOrSingleParentType::SingleParent => 350_000,
    }
}

/// 基礎控除の算出（所得水準に応じた逓減）
pub fn compute_basic_deduction(total_income: i64) -> i64 {
    if total_income <= 24_000_000 {
        480_000
    } else if total_income <= 24_500_000 {
        320_000
    } else if total_income <= 25_000_000 {
        160_000
    } else {
        0
    }
}

/// 全所得控除および税額控除を総合計算
pub fn compute_all_deductions(
    deductions: &TaxDeductions,
    total_income: i64,
) -> CalculatedDeductions {
    let basic = compute_basic_deduction(total_income);
    let social_insurance = deductions.social_insurance.max(0);
    let small_enterprise_mutual_aid = deductions.small_enterprise_mutual_aid.max(0);

    let life_insurance = compute_total_life_insurance_deduction(
        deductions.life_insurance_new_general,
        deductions.life_insurance_old_general,
        deductions.life_insurance_new_care,
        deductions.life_insurance_new_annuity,
        deductions.life_insurance_old_annuity,
    );

    let earthquake_insurance = compute_earthquake_insurance_deduction(
        deductions.earthquake_insurance,
        deductions.earthquake_old_long_term,
    );

    let medical = compute_medical_deduction(
        deductions.medical_expenses_paid,
        deductions.medical_expenses_reimbursed,
        total_income,
    );

    let donation = compute_donation_deduction(deductions.donations_paid, total_income);

    let spouse = compute_spouse_deduction(deductions.spouse_type, total_income);

    let dependents = compute_dependents_deduction(
        deductions.dependents_general_count,
        deductions.dependents_specific_count,
        deductions.dependents_elderly_living_count,
        deductions.dependents_elderly_other_count,
    );

    let disability = deductions.disability_deduction.max(0);

    let widow_or_single_parent =
        compute_widow_or_single_parent_deduction(deductions.widow_or_single_parent, total_income);

    let working_student = if deductions.working_student && total_income <= 750_000 {
        270_000
    } else {
        0
    };

    let total_deductions = basic
        + social_insurance
        + small_enterprise_mutual_aid
        + life_insurance
        + earthquake_insurance
        + medical
        + donation
        + spouse
        + dependents
        + disability
        + widow_or_single_parent
        + working_student;

    let housing_loan_tax_credit = deductions.housing_loan_tax_credit.max(0);
    let other_tax_credits = deductions.other_tax_credits.max(0);
    let total_tax_credits = housing_loan_tax_credit + other_tax_credits;

    CalculatedDeductions {
        basic,
        social_insurance,
        small_enterprise_mutual_aid,
        life_insurance,
        earthquake_insurance,
        medical,
        donation,
        spouse,
        dependents,
        disability,
        widow_or_single_parent,
        working_student,
        total_deductions,
        housing_loan_tax_credit,
        other_tax_credits,
        total_tax_credits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_life_insurance_brackets() {
        assert_eq!(compute_new_life_insurance_tier(0), 0);
        assert_eq!(compute_new_life_insurance_tier(15_000), 15_000);
        assert_eq!(compute_new_life_insurance_tier(20_000), 20_000);
        assert_eq!(compute_new_life_insurance_tier(30_000), 25_000); // 15k + 10k
        assert_eq!(compute_new_life_insurance_tier(40_000), 30_000); // 20k + 10k
        assert_eq!(compute_new_life_insurance_tier(60_000), 35_000); // 15k + 20k
        assert_eq!(compute_new_life_insurance_tier(80_000), 40_000);
        assert_eq!(compute_new_life_insurance_tier(120_000), 40_000); // 上限4万
    }

    #[test]
    fn test_old_life_insurance_brackets() {
        assert_eq!(compute_old_life_insurance_tier(0), 0);
        assert_eq!(compute_old_life_insurance_tier(20_000), 20_000);
        assert_eq!(compute_old_life_insurance_tier(25_000), 25_000);
        assert_eq!(compute_old_life_insurance_tier(40_000), 32_500); // 20k + 12.5k
        assert_eq!(compute_old_life_insurance_tier(50_000), 37_500); // 25k + 12.5k
        assert_eq!(compute_old_life_insurance_tier(80_000), 45_000); // 20k + 25k
        assert_eq!(compute_old_life_insurance_tier(100_000), 50_000);
        assert_eq!(compute_old_life_insurance_tier(150_000), 50_000); // 上限5万
    }

    #[test]
    fn test_life_insurance_combined_and_max() {
        // 新一般 50,000 (32,500) + 旧一般 60,000 (40,000)
        // 合算40,000 vs 旧40,000 => 40,000
        let gen = compute_combined_life_insurance(50_000, 60_000);
        assert_eq!(gen, 40_000);

        // 全枠満額の場合でも最高12万円
        let total = compute_total_life_insurance_deduction(
            100_000, // 40k
            0,
            100_000, // 40k
            100_000, // 40k
            0,
        );
        assert_eq!(total, 120_000);
    }

    #[test]
    fn test_earthquake_deduction() {
        assert_eq!(compute_earthquake_insurance_deduction(30_000, 0), 30_000);
        assert_eq!(compute_earthquake_insurance_deduction(60_000, 0), 50_000); // 上限5万
        // 旧長期 15,000 => 7,500 + 5,000 = 12,500
        assert_eq!(compute_earthquake_insurance_deduction(0, 15_000), 12_500);
        // 合算5万上限
        assert_eq!(compute_earthquake_insurance_deduction(40_000, 30_000), 50_000);
    }

    #[test]
    fn test_medical_deduction() {
        // 総所得 300万円 (5% = 15万円 > 10万円足切り) → 足切り10万円
        // 医療費 35万円, 保険補填 5万円 => 正味 30万円 - 10万円 = 20万円
        assert_eq!(compute_medical_deduction(350_000, 50_000, 3_000_000), 200_000);

        // 低所得者: 総所得 100万円 (5% = 5万円 < 10万円) → 足切り5万円
        // 医療費 12万円 => 12万円 - 5万円 = 7万円
        assert_eq!(compute_medical_deduction(120_000, 0, 1_000_000), 70_000);
    }

    #[test]
    fn test_donation_deduction() {
        // 総所得 500万円 (40% = 200万円)
        // ふるさと納税 60,000円 => 60,000 - 2,000 = 58,000円
        assert_eq!(compute_donation_deduction(60_000, 5_000_000), 58_000);
        assert_eq!(compute_donation_deduction(1_500, 5_000_000), 0);
    }

    #[test]
    fn test_dependents_deduction() {
        // 一般1人(38万), 特定1人(63万), 老人同居1人(58万)
        // 合計 = 380,000 + 630,000 + 580,000 = 1,590,000
        assert_eq!(
            compute_dependents_deduction(1, 1, 1, 0),
            1_590_000
        );
    }

    #[test]
    fn test_full_deductions_calculation() {
        let mut d = TaxDeductions::default();
        d.social_insurance = 600_000;
        d.small_enterprise_mutual_aid = 240_000; // iDeCo 2万/月
        d.life_insurance_new_general = 80_000; // 40,000
        d.donations_paid = 50_000; // 48,000
        d.housing_loan_tax_credit = 150_000;

        let total_income = 4_000_000;
        let calcs = compute_all_deductions(&d, total_income);

        assert_eq!(calcs.basic, 480_000);
        assert_eq!(calcs.social_insurance, 600_000);
        assert_eq!(calcs.small_enterprise_mutual_aid, 240_000);
        assert_eq!(calcs.life_insurance, 40_000);
        assert_eq!(calcs.donation, 48_000);
        assert_eq!(
            calcs.total_deductions,
            480_000 + 600_000 + 240_000 + 40_000 + 48_000
        );
        assert_eq!(calcs.housing_loan_tax_credit, 150_000);
        assert_eq!(calcs.total_tax_credits, 150_000);
    }
}
