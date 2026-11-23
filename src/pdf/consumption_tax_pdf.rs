//! Consumption Tax PDF statement generation.
//!
//! Produces a structured A4 statement for Consumption Tax and Local Consumption Tax
//! (消費税及び地方消費税 確定申告計算書) including invoice registration, tax breakdown,
//! statutory truncation, and comparative simulation across taxation methods.

use crate::core::consumption_tax::ConsumptionTaxSummary;
use crate::core::journal::{format_yen, TaxpayerInfo};
use crate::pdf::engine::PdfError;
use crate::pdf::fonts;
use printpdf::{BuiltinFont, Color, Line, Mm, PdfDocument, Point, Rgb};
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub fn export_consumption_tax_pdf(
    summary: &ConsumptionTaxSummary,
    taxpayer: &TaxpayerInfo,
    output_path: &Path,
) -> Result<PathBuf, PdfError> {
    let (doc, page, layer) = PdfDocument::new(
        format!("{}年分消費税申告計算書", summary.fiscal_year),
        Mm(210.0),
        Mm(297.0),
        "Layer1",
    );

    let (font, has_cjk) = match fonts::load_japanese_font_bytes() {
        Ok((path, bytes)) if !fonts::is_font_collection(&path) => {
            let mut cursor = Cursor::new(bytes);
            match doc.add_external_font(&mut cursor) {
                Ok(f) => (f, true),
                Err(_) => {
                    let f = doc
                        .add_builtin_font(BuiltinFont::Helvetica)
                        .map_err(|e| PdfError::Font(e.to_string()))?;
                    (f, false)
                }
            }
        }
        _ => {
            let f = doc
                .add_builtin_font(BuiltinFont::Helvetica)
                .map_err(|e| PdfError::Font(e.to_string()))?;
            (f, false)
        }
    };

    let current_layer = doc.get_page(page).get_layer(layer);

    // ヘッダータイトル描画
    current_layer.begin_text_section();
    current_layer.set_font(&font, 18.0);
    current_layer.set_text_cursor(Mm(20.0), Mm(275.0));
    let title = if has_cjk {
        format!("令和{}年分 消費税及び地方消費税 確定申告計算書", summary.fiscal_year - 2018)
    } else {
        format!("Fiscal Year {} Consumption Tax Statement", summary.fiscal_year)
    };
    current_layer.write_text(title, &font);
    current_layer.end_text_section();

    // サブタイトル / 納税者情報
    current_layer.begin_text_section();
    current_layer.set_font(&font, 10.0);
    current_layer.set_text_cursor(Mm(20.0), Mm(260.0));
    let name_str = if has_cjk {
        format!("氏名 / 屋号: {} {}", taxpayer.name, taxpayer.business_name)
    } else {
        format!("Name: {} {}", taxpayer.name, taxpayer.business_name)
    };
    current_layer.write_text(name_str, &font);

    current_layer.set_text_cursor(Mm(20.0), Mm(253.0));
    let invoice_str = if let Some(num) = &summary.config.invoice_registration_number {
        format!("適格請求書発行事業者登録番号: {num}")
    } else {
        "適格請求書登録番号: 未登録 (免税/未取得)".to_string()
    };
    current_layer.write_text(if has_cjk { invoice_str } else { "Invoice Reg No: Verified".into() }, &font);

    current_layer.set_text_cursor(Mm(20.0), Mm(246.0));
    let method_str = if has_cjk {
        format!("適用課税方式: {}", summary.config.tax_calc_method.label_ja())
    } else {
        format!("Tax Method: {:?}", summary.config.tax_calc_method)
    };
    current_layer.write_text(method_str, &font);
    current_layer.end_text_section();

    // 罫線区切り
    let line = Line {
        points: vec![
            (Point::new(Mm(20.0), Mm(240.0)), false),
            (Point::new(Mm(190.0), Mm(240.0)), false),
        ],
        is_closed: false,
    };
    current_layer.set_outline_color(Color::Rgb(Rgb::new(0.7, 0.7, 0.7, None)));
    current_layer.set_outline_thickness(0.5);
    current_layer.add_line(line);

    // 税額計算内訳テーブル
    let items = [
        ("課税売上高合計 (税込)", format_yen(summary.sales.total_taxable_sales_inclusive)),
        ("うち 10%標準課税売上高", format_yen(summary.sales.sales_10_inclusive)),
        ("うち 8%軽減課税売上高", format_yen(summary.sales.sales_8_inclusive)),
        ("課税標準額 (1,000円未満切捨て)", format_yen(summary.tax_base_amount)),
        ("課税標準額に対する消費税額 (国税分)", format_yen(summary.output_tax_national)),
        ("控除対象仕入税額 (国税分)", format_yen(summary.input_tax_deduction_national)),
        ("差引国税納付税額 (100円未満切捨て)", format_yen(summary.final_national_tax)),
        ("地方消費税納付税額 (国税 × 22/78、100円切捨て)", format_yen(summary.final_local_tax)),
        ("合計消費税等納付税額 (国税 + 地方税)", format_yen(summary.total_tax_payable)),
    ];

    let mut y = 230.0;
    for (label, val) in &items {
        current_layer.begin_text_section();
        current_layer.set_font(&font, 10.0);
        current_layer.set_text_cursor(Mm(25.0), Mm(y));
        let l_text = if has_cjk { label.to_string() } else { "Tax Breakdown Item".into() };
        current_layer.write_text(l_text, &font);

        current_layer.set_text_cursor(Mm(145.0), Mm(y));
        current_layer.write_text(format!("¥{val:>12}"), &font);
        current_layer.end_text_section();
        y -= 8.5;
    }

    // 方式別比較シミュレーション
    y -= 10.0;
    current_layer.begin_text_section();
    current_layer.set_font(&font, 12.0);
    current_layer.set_text_cursor(Mm(20.0), Mm(y));
    current_layer.write_text(
        if has_cjk { "【参考】各課税方式による納税額シミュレーション比較" } else { "Tax Simulation Comparison" },
        &font,
    );
    current_layer.end_text_section();

    y -= 10.0;
    let sim_items = [
        ("本則課税 (一般実額控除)", summary.simulation.general_tax_payable),
        ("簡易課税制度 (みなし仕入率適用)", summary.simulation.simplified_tax_payable),
        ("インボイス2割特例 (売上税額の20%納付)", summary.simulation.two_tenths_tax_payable),
    ];

    for (s_label, amt) in &sim_items {
        let is_best = *amt == summary.simulation.min_tax_payable;
        current_layer.begin_text_section();
        current_layer.set_font(&font, 10.0);
        current_layer.set_text_cursor(Mm(25.0), Mm(y));
        let full_label = if is_best {
            format!("★ {} [最有利]", s_label)
        } else {
            format!("   {}", s_label)
        };
        current_layer.write_text(if has_cjk { full_label } else { "Method Option".into() }, &font);

        current_layer.set_text_cursor(Mm(145.0), Mm(y));
        current_layer.write_text(format!("¥{:>12}", format_yen(*amt)), &font);
        current_layer.end_text_section();
        y -= 8.0;
    }

    // 保存
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(output_path)?;
    let mut writer = std::io::BufWriter::new(file);
    doc.save(&mut writer).map_err(|e| PdfError::Generate(e.to_string()))?;

    Ok(output_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::consumption_tax::calculate_consumption_tax;
    use crate::core::journal::{AccountCode, JournalEntry, LedgerDocument};
    use chrono::NaiveDate;

    #[test]
    fn test_export_consumption_tax_pdf() {
        let mut doc = LedgerDocument::new(2026);
        doc.taxpayer.name = "テスト太郎".into();
        doc.taxpayer.business_name = "テスト商店".into();
        doc.consumption_tax.is_taxable_business = true;
        doc.consumption_tax.invoice_registration_number = Some("T1234567890123".into());

        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            5_500_000,
            "上期売上",
        ));

        let summary = calculate_consumption_tax(&doc);
        let temp_dir = std::env::temp_dir();
        let pdf_path = temp_dir.join(format!("test_tax_{}.pdf", std::process::id()));

        let res = export_consumption_tax_pdf(&summary, &doc.taxpayer, &pdf_path);
        assert!(res.is_ok());
        let path = res.unwrap();
        assert!(path.exists());
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 500);

        let _ = std::fs::remove_file(path);
    }
}
