//! e-Tax (国税電子申告・納税システム) XML export engine.
//!
//! Generates valid XML structure conforming to National Tax Agency (国税庁)
//! e-Tax data specifications for Shotoku Kakutei Shinkoku Form B (所得税及び復興特別所得税の確定申告書B)
//! and Aoiro Shinkoku Kessansho (所得税青色申告決算書 一般用 全4面).

use crate::core::allocation::calculate_allocations;
use crate::core::depreciation::calculate_all_depreciations;
use crate::core::journal::LedgerDocument;
use crate::core::tax_calc::{
    aggregate_balance_sheet_breakdown, aggregate_monthly_breakdown,
    build_tax_report_with_deductions,
};
use chrono::Local;

/// XML特殊文字のエスケープ処理 (&, <, >, ", ')
pub fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// 帳票ドキュメントから国税庁 e-Tax 取り込み仕様に準拠した XML 文字列を生成。
pub fn generate_etax_xml(doc: &LedgerDocument) -> String {
    let report = build_tax_report_with_deductions(
        &doc.entries,
        doc.fiscal_year,
        doc.special_deduction,
        &doc.deductions,
    );
    let blue = &report.blue;
    let form_b = &report.form_b;
    let d_detail = &form_b.deductions_detail;
    let taxpayer = &doc.taxpayer;
    let monthly = aggregate_monthly_breakdown(&doc.entries);
    let bs = aggregate_balance_sheet_breakdown(&doc.entries, blue.ordinary_income_before_deduction);
    let depreciations = calculate_all_depreciations(doc);
    let allocations = calculate_allocations(doc);

    let created_at = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let mut xml = String::with_capacity(8192);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<e-TaxData xmlns=\"http://www.e-tax.nta.go.jp/schema/tax_return_v1\">\n");

    // 1. ヘッダー情報
    xml.push_str("  <Header>\n");
    xml.push_str("    <Software>TaxReturnApplication-Dioxus</Software>\n");
    xml.push_str("    <Version>1.0.0</Version>\n");
    xml.push_str(&format!("    <FiscalYear>{}</FiscalYear>\n", doc.fiscal_year));
    xml.push_str("    <DocumentType>ShotokuKakuteiShinkokuB_And_AoiroKessansho</DocumentType>\n");
    xml.push_str(&format!("    <CreateTimestamp>{}</CreateTimestamp>\n", created_at));
    xml.push_str("  </Header>\n");

    // 2. 納税者基本情報
    xml.push_str("  <Taxpayer>\n");
    xml.push_str(&format!("    <Name>{}</Name>\n", escape_xml(&taxpayer.name)));
    xml.push_str(&format!("    <BusinessName>{}</BusinessName>\n", escape_xml(&taxpayer.business_name)));
    xml.push_str(&format!("    <Address>{}</Address>\n", escape_xml(&taxpayer.address)));
    xml.push_str(&format!("    <Phone>{}</Phone>\n", escape_xml(&taxpayer.phone)));
    xml.push_str("  </Taxpayer>\n");

    // 3. 確定申告書B (ShotokuKakuteiShinkokuB)
    xml.push_str("  <ShotokuKakuteiShinkokuB>\n");

    // 収入金額等
    xml.push_str("    <Revenue>\n");
    xml.push_str(&format!("      <BusinessSales>{}</BusinessSales>\n", blue.sales));
    xml.push_str(&format!("      <TotalRevenue>{}</TotalRevenue>\n", blue.sales));
    xml.push_str("    </Revenue>\n");

    // 所得金額等
    xml.push_str("    <Income>\n");
    xml.push_str(&format!("      <BusinessIncome>{}</BusinessIncome>\n", form_b.business_income));
    xml.push_str(&format!("      <TotalIncome>{}</TotalIncome>\n", form_b.total_income));
    xml.push_str("    </Income>\n");

    // 所得から差し引かれる金額（所得控除）
    xml.push_str("    <Deductions>\n");
    xml.push_str(&format!("      <SocialInsurance>{}</SocialInsurance>\n", d_detail.social_insurance));
    xml.push_str(&format!("      <SmallEnterpriseMutualAid>{}</SmallEnterpriseMutualAid>\n", d_detail.small_enterprise_mutual_aid));
    xml.push_str(&format!("      <LifeInsurance>{}</LifeInsurance>\n", d_detail.life_insurance));
    xml.push_str(&format!("      <EarthquakeInsurance>{}</EarthquakeInsurance>\n", d_detail.earthquake_insurance));
    xml.push_str(&format!("      <WidowOrSingleParent>{}</WidowOrSingleParent>\n", d_detail.widow_or_single_parent));
    xml.push_str(&format!("      <WorkingStudent>{}</WorkingStudent>\n", d_detail.working_student));
    xml.push_str(&format!("      <Disability>{}</Disability>\n", d_detail.disability));
    xml.push_str(&format!("      <Spouse>{}</Spouse>\n", d_detail.spouse));
    xml.push_str(&format!("      <Dependents>{}</Dependents>\n", d_detail.dependents));
    xml.push_str(&format!("      <Basic>{}</Basic>\n", d_detail.basic));
    xml.push_str(&format!("      <Medical>{}</Medical>\n", d_detail.medical));
    xml.push_str(&format!("      <Donation>{}</Donation>\n", d_detail.donation));
    xml.push_str(&format!("      <TotalDeductions>{}</TotalDeductions>\n", form_b.total_deductions));
    xml.push_str("    </Deductions>\n");

    // 税金の計算
    xml.push_str("    <TaxCalculation>\n");
    xml.push_str(&format!("      <TaxableIncome>{}</TaxableIncome>\n", form_b.taxable_income));
    xml.push_str(&format!("      <TaxBeforeCredits>{}</TaxBeforeCredits>\n", form_b.tax_before_credits));
    xml.push_str(&format!("      <HousingLoanTaxCredit>{}</HousingLoanTaxCredit>\n", d_detail.housing_loan_tax_credit));
    xml.push_str(&format!("      <TotalTaxCredits>{}</TotalTaxCredits>\n", form_b.total_tax_credits));
    xml.push_str(&format!("      <IncomeTax>{}</IncomeTax>\n", form_b.income_tax));
    xml.push_str(&format!("      <ReconstructionSpecialSurtax>{}</ReconstructionSpecialSurtax>\n", form_b.reconstruction_surtax));
    xml.push_str(&format!("      <TotalIncomeTax>{}</TotalIncomeTax>\n", form_b.total_income_tax));
    xml.push_str(&format!("      <TaxPayable>{}</TaxPayable>\n", form_b.total_income_tax));
    xml.push_str("    </TaxCalculation>\n");
    xml.push_str("  </ShotokuKakuteiShinkokuB>\n");

    // 4. 青色申告決算書 (AoiroShinkokuKessansho)
    xml.push_str("  <AoiroShinkokuKessansho>\n");

    // 第1面: 損益計算書
    xml.push_str("    <Page1_ProfitAndLoss>\n");
    xml.push_str(&format!("      <Sales>{}</Sales>\n", blue.sales));
    xml.push_str(&format!("      <CostOfSales>{}</CostOfSales>\n", blue.cost_of_sales));
    xml.push_str(&format!("      <GrossProfit>{}</GrossProfit>\n", blue.gross_profit));
    xml.push_str("      <ItemizedExpenses>\n");
    for (name, amt) in &blue.itemized_expenses {
        xml.push_str(&format!(
            "        <ExpenseItem name=\"{}\">{}</ExpenseItem>\n",
            escape_xml(name),
            amt
        ));
    }
    xml.push_str("      </ItemizedExpenses>\n");
    xml.push_str(&format!("      <TotalExpenses>{}</TotalExpenses>\n", blue.total_expenses));
    xml.push_str(&format!("      <OrdinaryIncomeBeforeSpecialDeduction>{}</OrdinaryIncomeBeforeSpecialDeduction>\n", blue.ordinary_income_before_deduction));
    xml.push_str(&format!("      <BlueSpecialDeductionKind>{}</BlueSpecialDeductionKind>\n", blue.special_deduction_kind.label_ja()));
    xml.push_str(&format!("      <BlueSpecialDeductionAmount>{}</BlueSpecialDeductionAmount>\n", blue.special_deduction));
    xml.push_str(&format!("      <BusinessIncome>{}</BusinessIncome>\n", blue.business_income));
    xml.push_str("    </Page1_ProfitAndLoss>\n");

    // 第2面: 月別売上・仕入金額
    xml.push_str("    <Page2_MonthlyBreakdown>\n");
    xml.push_str("      <MonthlySales>\n");
    for m in 1..=12 {
        xml.push_str(&format!(
            "        <Month num=\"{}\">{}</Month>\n",
            m,
            monthly.monthly_sales[m - 1]
        ));
    }
    xml.push_str(&format!("        <TotalSales>{}</TotalSales>\n", monthly.total_sales));
    xml.push_str("      </MonthlySales>\n");
    xml.push_str("      <MonthlyPurchases>\n");
    for m in 1..=12 {
        xml.push_str(&format!(
            "        <Month num=\"{}\">{}</Month>\n",
            m,
            monthly.monthly_cogs[m - 1]
        ));
    }
    xml.push_str(&format!("        <TotalPurchases>{}</TotalPurchases>\n", monthly.total_cogs));
    xml.push_str("      </MonthlyPurchases>\n");
    xml.push_str("    </Page2_MonthlyBreakdown>\n");

    // 第3面: 減価償却費の計算 & 地代家賃の内訳
    xml.push_str("    <Page3_DepreciationAndRent>\n");
    xml.push_str("      <DepreciationAssets>\n");
    for dep in &depreciations {
        xml.push_str("        <Asset>\n");
        xml.push_str(&format!("          <Name>{}</Name>\n", escape_xml(&dep.name)));
        xml.push_str(&format!("          <AcquisitionDate>{}</AcquisitionDate>\n", dep.acquisition_date));
        xml.push_str(&format!("          <AcquisitionCost>{}</AcquisitionCost>\n", dep.acquisition_cost));
        xml.push_str(&format!("          <DepreciationMethod>{}</DepreciationMethod>\n", dep.depreciation_method.label_ja()));
        xml.push_str(&format!("          <UsefulLifeYears>{}</UsefulLifeYears>\n", dep.useful_life_years));
        xml.push_str(&format!("          <CurrentYearDepreciation>{}</CurrentYearDepreciation>\n", dep.ordinary_depreciation));
        xml.push_str(&format!("          <BusinessRatioPercent>{}</BusinessRatioPercent>\n", dep.business_ratio_percent));
        xml.push_str(&format!("          <BusinessExpenseAmount>{}</BusinessExpenseAmount>\n", dep.business_expense));
        xml.push_str(&format!("          <EndingBookValue>{}</EndingBookValue>\n", dep.closing_book_value));
        xml.push_str("        </Asset>\n");
    }
    xml.push_str("      </DepreciationAssets>\n");
    xml.push_str("      <HomeOfficeRentDetails>\n");
    for alloc in &allocations {
        if alloc.account == crate::core::journal::AccountCode::Rent {
            xml.push_str("        <RentAllocation>\n");
            xml.push_str(&format!("          <Account>{}</Account>\n", alloc.account_label));
            xml.push_str(&format!("          <SubAccount>{}</SubAccount>\n", escape_xml(alloc.sub_account.as_deref().unwrap_or("一般"))));
            xml.push_str(&format!("          <TotalPaid>{}</TotalPaid>\n", alloc.total_expense));
            xml.push_str(&format!("          <BusinessRatioPercent>{}</BusinessRatioPercent>\n", alloc.business_ratio_percent));
            xml.push_str(&format!("          <BusinessExpenseAmount>{}</BusinessExpenseAmount>\n", alloc.business_expense));
            xml.push_str(&format!("          <PersonalDrawingsAmount>{}</PersonalDrawingsAmount>\n", alloc.personal_expense));
            xml.push_str("        </RentAllocation>\n");
        }
    }
    xml.push_str("      </HomeOfficeRentDetails>\n");
    xml.push_str("    </Page3_DepreciationAndRent>\n");

    // 第4面: 貸借対照表
    xml.push_str("    <Page4_BalanceSheet>\n");
    xml.push_str("      <Assets>\n");
    for item in &bs.assets {
        xml.push_str(&format!(
            "        <Item label=\"{}\" opening=\"{}\" closing=\"{}\" />\n",
            escape_xml(&item.label),
            item.opening,
            item.closing
        ));
    }
    xml.push_str(&format!("        <TotalAssets opening=\"{}\" closing=\"{}\" />\n", bs.total_assets_opening, bs.total_assets_closing));
    xml.push_str("      </Assets>\n");
    xml.push_str("      <LiabilitiesAndEquity>\n");
    for item in &bs.liabilities {
        xml.push_str(&format!(
            "        <Liability label=\"{}\" opening=\"{}\" closing=\"{}\" />\n",
            escape_xml(&item.label),
            item.opening,
            item.closing
        ));
    }
    xml.push_str(&format!("        <TotalLiabilities opening=\"{}\" closing=\"{}\" />\n", bs.total_liabilities_opening, bs.total_liabilities_closing));
    for item in &bs.equity {
        xml.push_str(&format!(
            "        <Equity label=\"{}\" opening=\"{}\" closing=\"{}\" />\n",
            escape_xml(&item.label),
            item.opening,
            item.closing
        ));
    }
    xml.push_str(&format!("        <TotalEquity opening=\"{}\" closing=\"{}\" />\n", bs.total_equity_opening, bs.total_equity_closing));
    xml.push_str(&format!("        <TotalLiabilitiesAndEquity opening=\"{}\" closing=\"{}\" />\n", bs.total_liabilities_and_equity_opening, bs.total_liabilities_and_equity_closing));
    xml.push_str("      </LiabilitiesAndEquity>\n");
    xml.push_str("    </Page4_BalanceSheet>\n");

    xml.push_str("  </AoiroShinkokuKessansho>\n");
    xml.push_str("</e-TaxData>\n");

    xml
}

/// e-Tax XML の妥当性（XMLヘッダー、主要タグの対構造）を検証。
pub fn validate_etax_xml(xml: &str) -> Result<(), String> {
    if !xml.starts_with("<?xml version=\"1.0\"") {
        return Err("XML宣言で始まっていません。".into());
    }
    let required_tags = [
        "<e-TaxData",
        "</e-TaxData>",
        "<Header>",
        "</Header>",
        "<Taxpayer>",
        "</Taxpayer>",
        "<ShotokuKakuteiShinkokuB>",
        "</ShotokuKakuteiShinkokuB>",
        "<AoiroShinkokuKessansho>",
        "</AoiroShinkokuKessansho>",
        "<Page1_ProfitAndLoss>",
        "</Page1_ProfitAndLoss>",
        "<Page2_MonthlyBreakdown>",
        "</Page2_MonthlyBreakdown>",
        "<Page3_DepreciationAndRent>",
        "</Page3_DepreciationAndRent>",
        "<Page4_BalanceSheet>",
        "</Page4_BalanceSheet>",
    ];

    for tag in required_tags {
        if !xml.contains(tag) {
            return Err(format!("必須XMLタグが見つかりません: {tag}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::deductions::TaxDeductions;
    use crate::core::journal::{AccountCode, JournalEntry, TaxpayerInfo};
    use crate::core::tax_calc::BlueSpecialDeductionKind;
    use chrono::NaiveDate;

    #[test]
    fn test_escape_xml() {
        let text = r#"山田 & 田中 <"商店'> & Co."#;
        let escaped = escape_xml(text);
        assert_eq!(escaped, "山田 &amp; 田中 &lt;&quot;商店&apos;&gt; &amp; Co.");
    }

    #[test]
    fn test_generate_and_validate_etax_xml() {
        let mut doc = LedgerDocument::new(2025);
        doc.taxpayer = TaxpayerInfo {
            name: "佐藤 健一 & 家族".into(),
            business_name: "佐藤テクノロジー <IT>".into(),
            address: "東京都港区六本木1-2-3".into(),
            phone: "03-1234-5678".into(),
        };
        doc.special_deduction = BlueSpecialDeductionKind::Yen650k;
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            4_000_000,
            "システム受託開発売上",
        ));
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 5, 10).unwrap(),
            AccountCode::Supplies,
            AccountCode::Bank,
            200_000,
            "開発用PC周辺機器",
        ));

        let mut deductions = TaxDeductions::default();
        deductions.social_insurance = 300_000;
        doc.deductions = deductions;

        let xml = generate_etax_xml(&doc);
        assert!(xml.contains("<Software>TaxReturnApplication-Dioxus</Software>"));
        assert!(xml.contains("<Name>佐藤 健一 &amp; 家族</Name>"));
        assert!(xml.contains("<BusinessName>佐藤テクノロジー &lt;IT&gt;</BusinessName>"));
        assert!(xml.contains("<Sales>4000000</Sales>"));
        assert!(xml.contains("<TotalExpenses>200000</TotalExpenses>"));
        assert!(xml.contains("<SocialInsurance>300000</SocialInsurance>"));
        assert!(xml.contains("<BlueSpecialDeductionAmount>650000</BlueSpecialDeductionAmount>"));

        let val_res = validate_etax_xml(&xml);
        assert!(val_res.is_ok(), "Validation failed: {:?}", val_res.err());
    }
}
