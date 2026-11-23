//! Official tax return PDF overlay and generator.
//!
//! Generates full multi-page official filings matching National Tax Agency (国税庁) layouts:
//! - 所得税青色申告決算書（一般用）: 4 pages (P&L, Monthly Breakdown, Depreciation & Rent, Balance Sheet)
//! - 確定申告書B: 2 pages (Page 1: Income, Deductions & Taxes; Page 2: Breakdown & Dependents)

use crate::core::allocation::calculate_allocations;
use crate::core::depreciation::calculate_all_depreciations;
use crate::core::journal::{format_yen, AccountCode, LedgerDocument};
use crate::core::tax_calc::{
    aggregate_balance_sheet_breakdown, aggregate_monthly_breakdown,
    build_tax_report_with_deductions,
};
use crate::pdf::engine::PdfError;
use crate::pdf::fonts;
use printpdf::path::PaintMode;
use printpdf::{
    BuiltinFont, Color, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerReference, Point, Rect,
    Rgb,
};
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};

struct PdfContext {
    doc: printpdf::PdfDocumentReference,
    font: IndirectFontRef,
    has_cjk_font: bool,
}

fn init_pdf(title: &str) -> Result<(PdfContext, printpdf::PdfPageIndex, printpdf::PdfLayerIndex), PdfError> {
    let (doc, page, layer) = PdfDocument::new(title, Mm(210.0), Mm(297.0), "Layer 1");

    let (font, has_cjk_font) = match fonts::load_japanese_font_bytes() {
        Ok((path, bytes)) if !fonts::is_font_collection(&path) => {
            let mut cursor = Cursor::new(bytes);
            match doc.add_external_font(&mut cursor) {
                Ok(font) => (font, true),
                Err(e) => {
                    return Err(PdfError::Font(format!(
                        "フォント埋め込みに失敗しました ({}): {e}",
                        path.display()
                    )));
                }
            }
        }
        _ => {
            let font = doc
                .add_builtin_font(BuiltinFont::Helvetica)
                .map_err(|e| PdfError::Font(e.to_string()))?;
            (font, false)
        }
    };

    Ok((
        PdfContext {
            doc,
            font,
            has_cjk_font,
        },
        page,
        layer,
    ))
}

fn ascii_fallback(text: &str) -> String {
    let filtered: String = text
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    if filtered.contains('?') {
        let digits: String = text
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == ',' || *c == '-' || *c == '¥' || *c == '.')
            .collect();
        if !digits.is_empty() {
            return digits;
        }
    }
    filtered
}

fn draw_rect(
    layer: &PdfLayerReference,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    stroke: bool,
    fill_color: Option<(f32, f32, f32)>,
) {
    if let Some((r, g, b)) = fill_color {
        layer.set_fill_color(Color::Rgb(Rgb::new(r, g, b, None)));
    }
    if stroke {
        layer.set_outline_color(Color::Rgb(Rgb::new(0.3, 0.3, 0.3, None)));
        layer.set_outline_thickness(0.4);
    }
    let mode = match (fill_color.is_some(), stroke) {
        (true, true) => PaintMode::FillStroke,
        (true, false) => PaintMode::Fill,
        (false, true) => PaintMode::Stroke,
        (false, false) => PaintMode::Clip,
    };
    let rect = Rect::new(Mm(x), Mm(y - h), Mm(x + w), Mm(y)).with_mode(mode);
    layer.add_rect(rect);
}

fn draw_line(layer: &PdfLayerReference, x1: f32, y1: f32, x2: f32, y2: f32) {
    layer.set_outline_color(Color::Rgb(Rgb::new(0.4, 0.4, 0.4, None)));
    layer.set_outline_thickness(0.3);
    let line = Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y1)), false),
            (Point::new(Mm(x2), Mm(y2)), false),
        ],
        is_closed: false,
    };
    layer.add_line(line);
}

fn draw_text(
    layer: &PdfLayerReference,
    ctx: &PdfContext,
    text: &str,
    size: f32,
    x: f32,
    y: f32,
) {
    let rendered = if ctx.has_cjk_font {
        text.to_string()
    } else {
        ascii_fallback(text)
    };
    layer.use_text(rendered, size, Mm(x), Mm(y), &ctx.font);
}

fn yen(v: i64) -> String {
    format!("¥{}", format_yen(v))
}

/// 青色申告決算書（一般用）公式4ページPDFのエクスポート
pub fn export_blue_return_official_pdf(
    doc: &LedgerDocument,
    output_path: &Path,
) -> Result<PathBuf, PdfError> {
    let report = build_tax_report_with_deductions(
        &doc.entries,
        doc.fiscal_year,
        doc.special_deduction,
        &doc.deductions,
    );
    let blue = &report.blue;
    let taxpayer = &doc.taxpayer;
    let monthly = aggregate_monthly_breakdown(&doc.entries);
    let bs = aggregate_balance_sheet_breakdown(&doc.entries, blue.ordinary_income_before_deduction);
    let depreciations = calculate_all_depreciations(doc);
    let allocations = calculate_allocations(doc);

    let (ctx, page1, layer1) = init_pdf("令和分 所得税青色申告決算書（一般用）")?;

    // ==========================================
    // PAGE 1: 損益計算書 (Profit and Loss Statement)
    // ==========================================
    {
        let layer = ctx.doc.get_page(page1).get_layer(layer1);

        // 外枠
        draw_rect(&layer, 12.0, 285.0, 186.0, 270.0, true, None);

        // 表題ヘッダー帯
        draw_rect(&layer, 12.0, 285.0, 186.0, 14.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(
            &layer,
            &ctx,
            &format!("令和 {} 年分 所得税青色申告決算書（一般用） 【損益計算書】", doc.fiscal_year),
            12.0,
            20.0,
            276.0,
        );

        // 納税者情報ボックス
        draw_rect(&layer, 15.0, 268.0, 180.0, 16.0, true, None);
        draw_text(&layer, &ctx, &format!("氏名: {}", if taxpayer.name.is_empty() { "（未入力）" } else { &taxpayer.name }), 9.0, 18.0, 262.0);
        draw_text(&layer, &ctx, &format!("屋号: {}", if taxpayer.business_name.is_empty() { "（未入力）" } else { &taxpayer.business_name }), 9.0, 80.0, 262.0);
        draw_text(&layer, &ctx, &format!("住所: {}", if taxpayer.address.is_empty() { "（未入力）" } else { &taxpayer.address }), 9.0, 18.0, 255.0);
        draw_text(&layer, &ctx, &format!("自 令和{}年1月1日 至 12月31日", doc.fiscal_year), 8.5, 130.0, 255.0);

        // 売上・仕入・利益セクション
        draw_rect(&layer, 15.0, 248.0, 180.0, 22.0, true, Some((0.96, 0.97, 0.98)));
        draw_text(&layer, &ctx, "売上（収入）金額 [1]:", 9.0, 18.0, 242.0);
        draw_text(&layer, &ctx, &yen(blue.sales), 9.5, 65.0, 242.0);
        draw_text(&layer, &ctx, "売上原価（仕入） [9]:", 9.0, 105.0, 242.0);
        draw_text(&layer, &ctx, &yen(blue.cost_of_sales), 9.5, 155.0, 242.0);

        draw_text(&layer, &ctx, "差引金額（売上総利益） [10]:", 9.5, 18.0, 231.0);
        draw_text(&layer, &ctx, &yen(blue.gross_profit), 10.0, 80.0, 231.0);

        // 経費グリッドヘッダー
        draw_rect(&layer, 15.0, 222.0, 180.0, 8.0, true, Some((0.90, 0.93, 0.96)));
        draw_text(&layer, &ctx, "経費の科目及び金額 [11]〜[31]", 9.0, 18.0, 217.0);

        // 経費明細リスト描画
        let y = 208.0;
        let expenses_list = if !blue.itemized_expenses.is_empty() {
            blue.itemized_expenses.clone()
        } else {
            blue.expenses.lines().into_iter().map(|(k, v)| (k.to_string(), v)).collect()
        };

        for (idx, (name, amount)) in expenses_list.iter().take(14).enumerate() {
            let col_x = if idx % 2 == 0 { 18.0 } else { 105.0 };
            let curr_y = y - ((idx / 2) as f32 * 8.5);
            draw_text(&layer, &ctx, &format!("{}:", name), 8.5, col_x, curr_y);
            draw_text(&layer, &ctx, &yen(*amount), 8.5, col_x + 48.0, curr_y);
            draw_line(&layer, col_x, curr_y - 2.0, col_x + 82.0, curr_y - 2.0);
        }

        // 経費合計・所得計算ボックス
        draw_rect(&layer, 15.0, 138.0, 180.0, 48.0, true, Some((0.97, 0.98, 1.0)));
        draw_text(&layer, &ctx, "経費合計 [32]:", 9.5, 20.0, 131.0);
        draw_text(&layer, &ctx, &yen(blue.total_expenses), 9.5, 90.0, 131.0);

        draw_text(&layer, &ctx, "青色申告特別控除前の所得金額 [43]:", 9.5, 20.0, 119.0);
        draw_text(&layer, &ctx, &yen(blue.ordinary_income_before_deduction), 9.5, 90.0, 119.0);

        draw_text(&layer, &ctx, &format!("青色申告特別控除額（{}） [44]:", blue.special_deduction_kind.label_ja()), 9.5, 20.0, 107.0);
        draw_text(&layer, &ctx, &yen(blue.special_deduction), 9.5, 90.0, 107.0);

        draw_text(&layer, &ctx, "所得金額 [45]:", 11.0, 20.0, 95.0);
        draw_text(&layer, &ctx, &yen(blue.business_income), 11.5, 90.0, 95.0);

        // ページ1 フッター
        draw_text(&layer, &ctx, "所得税青色申告決算書（一般用） 第1面 損益計算書", 8.0, 80.0, 20.0);
    }

    // ==========================================
    // PAGE 2: 月別売上・仕入金額 (Monthly Breakdown)
    // ==========================================
    let (page2, layer2) = ctx.doc.add_page(Mm(210.0), Mm(297.0), "Layer 2");
    {
        let layer = ctx.doc.get_page(page2).get_layer(layer2);
        draw_rect(&layer, 12.0, 285.0, 186.0, 270.0, true, None);
        draw_rect(&layer, 12.0, 285.0, 186.0, 14.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(
            &layer,
            &ctx,
            &format!("令和 {} 年分 所得税青色申告決算書（一般用） 【第2面 月別売上・仕入金額】", doc.fiscal_year),
            12.0,
            20.0,
            276.0,
        );

        // 月別テーブルヘッダー
        draw_rect(&layer, 18.0, 260.0, 174.0, 10.0, true, Some((0.88, 0.92, 0.96)));
        draw_text(&layer, &ctx, "月別", 9.0, 24.0, 254.0);
        draw_text(&layer, &ctx, "売上（収入）金額 (円)", 9.0, 60.0, 254.0);
        draw_text(&layer, &ctx, "仕入金額 (円)", 9.0, 130.0, 254.0);

        let mut y = 245.0;
        for m in 1..=12 {
            let row_bg = if m % 2 == 0 { Some((0.97, 0.97, 0.97)) } else { None };
            draw_rect(&layer, 18.0, y + 4.0, 174.0, 11.0, true, row_bg);
            draw_text(&layer, &ctx, &format!("{}月", m), 8.5, 24.0, y - 2.0);
            draw_text(&layer, &ctx, &yen(monthly.monthly_sales[m - 1]), 8.5, 60.0, y - 2.0);
            draw_text(&layer, &ctx, &yen(monthly.monthly_cogs[m - 1]), 8.5, 130.0, y - 2.0);
            y -= 11.0;
        }

        // 合計行
        draw_rect(&layer, 18.0, y + 4.0, 174.0, 12.0, true, Some((0.90, 0.94, 0.98)));
        draw_text(&layer, &ctx, "計", 9.5, 24.0, y - 3.0);
        draw_text(&layer, &ctx, &yen(monthly.total_sales), 9.5, 60.0, y - 3.0);
        draw_text(&layer, &ctx, &yen(monthly.total_cogs), 9.5, 130.0, y - 3.0);

        // ページ2 フッター
        draw_text(&layer, &ctx, "所得税青色申告決算書（一般用） 第2面 月別売上（収入）金額及び仕入金額", 8.0, 68.0, 20.0);
    }

    // ==========================================
    // PAGE 3: 減価償却費の計算 及び 地代家賃の内訳
    // ==========================================
    let (page3, layer3) = ctx.doc.add_page(Mm(210.0), Mm(297.0), "Layer 3");
    {
        let layer = ctx.doc.get_page(page3).get_layer(layer3);
        draw_rect(&layer, 12.0, 285.0, 186.0, 270.0, true, None);
        draw_rect(&layer, 12.0, 285.0, 186.0, 14.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(
            &layer,
            &ctx,
            &format!("令和 {} 年分 所得税青色申告決算書（一般用） 【第3面 減価償却・地代家賃】", doc.fiscal_year),
            12.0,
            20.0,
            276.0,
        );

        // 減価償却費の計算セクション
        draw_rect(&layer, 15.0, 265.0, 180.0, 8.0, true, Some((0.90, 0.93, 0.96)));
        draw_text(&layer, &ctx, "減価償却費の計算", 9.5, 20.0, 260.0);

        draw_rect(&layer, 15.0, 255.0, 180.0, 8.0, true, Some((0.95, 0.95, 0.95)));
        draw_text(&layer, &ctx, "資産の名称", 8.0, 17.0, 250.0);
        draw_text(&layer, &ctx, "取得価額", 8.0, 65.0, 250.0);
        draw_text(&layer, &ctx, "償却方法", 8.0, 95.0, 250.0);
        draw_text(&layer, &ctx, "本年償却額", 8.0, 130.0, 250.0);
        draw_text(&layer, &ctx, "期末残高", 8.0, 165.0, 250.0);

        let mut dep_y = 242.0;
        if depreciations.is_empty() {
            draw_text(&layer, &ctx, "（登録されている減価償却資産はありません）", 8.5, 20.0, dep_y);
        } else {
            for dep in depreciations.iter().take(6) {
                draw_text(&layer, &ctx, &dep.name, 8.0, 17.0, dep_y);
                draw_text(&layer, &ctx, &yen(dep.acquisition_cost), 8.0, 65.0, dep_y);
                draw_text(&layer, &ctx, dep.depreciation_method.label_ja(), 7.5, 95.0, dep_y);
                draw_text(&layer, &ctx, &yen(dep.business_expense), 8.0, 130.0, dep_y);
                draw_text(&layer, &ctx, &yen(dep.closing_book_value), 8.0, 165.0, dep_y);
                draw_line(&layer, 15.0, dep_y - 2.0, 195.0, dep_y - 2.0);
                dep_y -= 9.0;
            }
        }

        // 地代家賃の内訳セクション
        let rent_y = 150.0;
        draw_rect(&layer, 15.0, rent_y, 180.0, 8.0, true, Some((0.90, 0.93, 0.96)));
        draw_text(&layer, &ctx, "地代家賃の内訳（家事按分）", 9.5, 20.0, rent_y - 5.5);

        draw_rect(&layer, 15.0, rent_y - 10.0, 180.0, 8.0, true, Some((0.95, 0.95, 0.95)));
        draw_text(&layer, &ctx, "賃借物件・勘定", 8.0, 17.0, rent_y - 15.0);
        draw_text(&layer, &ctx, "年間支払総額", 8.0, 70.0, rent_y - 15.0);
        draw_text(&layer, &ctx, "事業割合", 8.0, 115.0, rent_y - 15.0);
        draw_text(&layer, &ctx, "必要経費算入額", 8.0, 150.0, rent_y - 15.0);

        let mut r_y = rent_y - 26.0;
        let rent_allocations: Vec<_> = allocations.iter().filter(|a| a.account == AccountCode::Rent).collect();
        if rent_allocations.is_empty() {
            draw_text(&layer, &ctx, "（地代家賃の按分設定はありません）", 8.5, 20.0, r_y);
        } else {
            for alloc in rent_allocations {
                draw_text(&layer, &ctx, &format!("{} ({})", alloc.account_label, alloc.sub_account.as_deref().unwrap_or("一般")), 8.0, 17.0, r_y);
                draw_text(&layer, &ctx, &yen(alloc.total_expense), 8.0, 70.0, r_y);
                draw_text(&layer, &ctx, &format!("{}%", alloc.business_ratio_percent), 8.0, 115.0, r_y);
                draw_text(&layer, &ctx, &yen(alloc.business_expense), 8.0, 150.0, r_y);
                draw_line(&layer, 15.0, r_y - 2.0, 195.0, r_y - 2.0);
                r_y -= 9.0;
            }
        }

        // ページ3 フッター
        draw_text(&layer, &ctx, "所得税青色申告決算書（一般用） 第3面 減価償却費の計算・地代家賃の内訳", 8.0, 65.0, 20.0);
    }

    // ==========================================
    // PAGE 4: 貸借対照表 (Balance Sheet)
    // ==========================================
    let (page4, layer4) = ctx.doc.add_page(Mm(210.0), Mm(297.0), "Layer 4");
    {
        let layer = ctx.doc.get_page(page4).get_layer(layer4);
        draw_rect(&layer, 12.0, 285.0, 186.0, 270.0, true, None);
        draw_rect(&layer, 12.0, 285.0, 186.0, 14.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(
            &layer,
            &ctx,
            &format!("令和 {} 年分 所得税青色申告決算書（一般用） 【第4面 貸借対照表】", doc.fiscal_year),
            12.0,
            20.0,
            276.0,
        );

        // 資産の部（左） / 負債・資本の部（右） ヘッダー
        draw_rect(&layer, 15.0, 265.0, 88.0, 8.0, true, Some((0.88, 0.92, 0.96)));
        draw_text(&layer, &ctx, "資産の部", 9.0, 20.0, 260.0);
        draw_text(&layer, &ctx, "期首(1/1)", 8.0, 50.0, 260.0);
        draw_text(&layer, &ctx, "期末(12/31)", 8.0, 75.0, 260.0);

        draw_rect(&layer, 107.0, 265.0, 88.0, 8.0, true, Some((0.88, 0.92, 0.96)));
        draw_text(&layer, &ctx, "負債・資本の部", 9.0, 110.0, 260.0);
        draw_text(&layer, &ctx, "期首(1/1)", 8.0, 142.0, 260.0);
        draw_text(&layer, &ctx, "期末(12/31)", 8.0, 168.0, 260.0);

        // 資産の部リスト
        let mut y_asset = 250.0;
        for item in &bs.assets {
            draw_text(&layer, &ctx, &item.label, 8.0, 17.0, y_asset);
            draw_text(&layer, &ctx, &yen(item.opening), 7.5, 48.0, y_asset);
            draw_text(&layer, &ctx, &yen(item.closing), 7.5, 73.0, y_asset);
            draw_line(&layer, 15.0, y_asset - 2.0, 103.0, y_asset - 2.0);
            y_asset -= 8.5;
        }

        // 資産合計
        draw_rect(&layer, 15.0, 75.0, 88.0, 10.0, true, Some((0.92, 0.94, 0.98)));
        draw_text(&layer, &ctx, "資産合計", 9.0, 18.0, 69.0);
        draw_text(&layer, &ctx, &yen(bs.total_assets_opening), 8.5, 48.0, 69.0);
        draw_text(&layer, &ctx, &yen(bs.total_assets_closing), 8.5, 73.0, 69.0);

        // 負債・資本の部リスト
        let mut y_liab = 250.0;
        for item in &bs.liabilities {
            draw_text(&layer, &ctx, &item.label, 8.0, 110.0, y_liab);
            draw_text(&layer, &ctx, &yen(item.opening), 7.5, 140.0, y_liab);
            draw_text(&layer, &ctx, &yen(item.closing), 7.5, 166.0, y_liab);
            draw_line(&layer, 107.0, y_liab - 2.0, 195.0, y_liab - 2.0);
            y_liab -= 8.5;
        }
        for item in &bs.equity {
            draw_text(&layer, &ctx, &item.label, 7.5, 110.0, y_liab);
            draw_text(&layer, &ctx, &yen(item.opening), 7.5, 140.0, y_liab);
            draw_text(&layer, &ctx, &yen(item.closing), 7.5, 166.0, y_liab);
            draw_line(&layer, 107.0, y_liab - 2.0, 195.0, y_liab - 2.0);
            y_liab -= 8.5;
        }

        // 負債・資本合計
        draw_rect(&layer, 107.0, 75.0, 88.0, 10.0, true, Some((0.92, 0.94, 0.98)));
        draw_text(&layer, &ctx, "負債・資本合計", 9.0, 110.0, 69.0);
        draw_text(&layer, &ctx, &yen(bs.total_liabilities_and_equity_opening), 8.5, 140.0, 69.0);
        draw_text(&layer, &ctx, &yen(bs.total_liabilities_and_equity_closing), 8.5, 166.0, 69.0);

        // ページ4 フッター
        draw_text(&layer, &ctx, "所得税青色申告決算書（一般用） 第4面 貸借対照表", 8.0, 75.0, 20.0);
    }

    save_doc(ctx, output_path)
}

/// 確定申告書B公式2ページPDFのエクスポート
pub fn export_form_b_official_pdf(
    doc: &LedgerDocument,
    output_path: &Path,
) -> Result<PathBuf, PdfError> {
    let report = build_tax_report_with_deductions(
        &doc.entries,
        doc.fiscal_year,
        doc.special_deduction,
        &doc.deductions,
    );
    let blue = &report.blue;
    let form_b = &report.form_b;
    let d = &form_b.deductions_detail;
    let taxpayer = &doc.taxpayer;

    let (ctx, page1, layer1) = init_pdf("令和分 確定申告書B")?;

    // ==========================================
    // PAGE 1: 第一面（収入・所得・控除・税金の計算）
    // ==========================================
    {
        let layer = ctx.doc.get_page(page1).get_layer(layer1);
        draw_rect(&layer, 12.0, 285.0, 186.0, 270.0, true, None);
        draw_rect(&layer, 12.0, 285.0, 186.0, 14.0, true, Some((0.93, 0.96, 0.94)));
        draw_text(
            &layer,
            &ctx,
            &format!("令和 {} 年分の所得税及び復興特別所得税の確定申告書B 【第一面】", doc.fiscal_year),
            12.0,
            20.0,
            276.0,
        );

        // 納税者基本情報
        draw_rect(&layer, 15.0, 268.0, 180.0, 16.0, true, None);
        draw_text(&layer, &ctx, &format!("住所: {}", if taxpayer.address.is_empty() { "（未入力）" } else { &taxpayer.address }), 9.0, 18.0, 262.0);
        draw_text(&layer, &ctx, &format!("氏名: {}", if taxpayer.name.is_empty() { "（未入力）" } else { &taxpayer.name }), 9.0, 18.0, 255.0);
        draw_text(&layer, &ctx, &format!("屋号: {}", if taxpayer.business_name.is_empty() { "（未入力）" } else { &taxpayer.business_name }), 9.0, 95.0, 255.0);
        draw_text(&layer, &ctx, &format!("電話: {}", if taxpayer.phone.is_empty() { "（未入力）" } else { &taxpayer.phone }), 8.5, 140.0, 255.0);

        // 収入・所得ブロック（左上）
        draw_rect(&layer, 15.0, 248.0, 88.0, 8.0, true, Some((0.94, 0.96, 0.92)));
        draw_text(&layer, &ctx, "収入金額等・所得金額等", 9.0, 18.0, 243.0);

        draw_text(&layer, &ctx, "事業営業等 収入 [ア]:", 8.5, 18.0, 233.0);
        draw_text(&layer, &ctx, &yen(blue.sales), 8.5, 65.0, 233.0);

        draw_text(&layer, &ctx, "事業営業等 所得 [①]:", 8.5, 18.0, 222.0);
        draw_text(&layer, &ctx, &yen(form_b.business_income), 8.5, 65.0, 222.0);

        draw_text(&layer, &ctx, "所得金額合計 [⑨]:", 9.0, 18.0, 211.0);
        draw_text(&layer, &ctx, &yen(form_b.total_income), 9.0, 65.0, 211.0);

        // 税金の計算ブロック（右上）
        draw_rect(&layer, 107.0, 248.0, 88.0, 8.0, true, Some((0.98, 0.94, 0.92)));
        draw_text(&layer, &ctx, "税金の計算", 9.0, 110.0, 243.0);

        draw_text(&layer, &ctx, "課税される所得金額 [㉓]:", 8.5, 110.0, 233.0);
        draw_text(&layer, &ctx, &yen(form_b.taxable_income), 8.5, 160.0, 233.0);

        draw_text(&layer, &ctx, "上の23に対する税額 [㉔]:", 8.5, 110.0, 222.0);
        draw_text(&layer, &ctx, &yen(form_b.tax_before_credits), 8.5, 160.0, 222.0);

        draw_text(&layer, &ctx, "住宅借入金等特別控除 [㉖]:", 8.5, 110.0, 211.0);
        draw_text(&layer, &ctx, &yen(d.housing_loan_tax_credit), 8.5, 160.0, 211.0);

        draw_text(&layer, &ctx, "差引所得税額 [㉗]:", 8.5, 110.0, 200.0);
        draw_text(&layer, &ctx, &yen(form_b.income_tax), 8.5, 160.0, 200.0);

        draw_text(&layer, &ctx, "復興特別所得税額 [㉚]:", 8.5, 110.0, 189.0);
        draw_text(&layer, &ctx, &yen(form_b.reconstruction_surtax), 8.5, 160.0, 189.0);

        draw_text(&layer, &ctx, "申告納税額 [㉝]:", 10.0, 110.0, 175.0);
        draw_text(&layer, &ctx, &yen(form_b.total_income_tax), 10.5, 155.0, 175.0);

        // 所得から差し引かれる金額（所得控除）一覧ブロック（下段）
        draw_rect(&layer, 15.0, 195.0, 88.0, 8.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(&layer, &ctx, "所得から差し引かれる金額（所得控除）", 8.5, 18.0, 190.0);

        let deductions_rows = [
            ("社会保険料控除 [⑩]", d.social_insurance),
            ("小規模企業共済等掛金控除 [⑪]", d.small_enterprise_mutual_aid),
            ("生命保険料控除 [⑫]", d.life_insurance),
            ("地震保険料控除 [⑬]", d.earthquake_insurance),
            ("寡婦・ひとり親控除 [⑭]", d.widow_or_single_parent),
            ("勤労学生控除 [⑮]", d.working_student),
            ("障害者控除 [⑯]", d.disability),
            ("配偶者（特別）控除 [⑰]", d.spouse),
            ("扶養控除 [⑱]", d.dependents),
            ("基礎控除 [⑲]", d.basic),
            ("医療費控除 [⑳]", d.medical),
            ("寄附金控除 [㉑]", d.donation),
        ];

        let mut d_y = 180.0;
        for (label, amount) in deductions_rows {
            draw_text(&layer, &ctx, label, 7.5, 18.0, d_y);
            draw_text(&layer, &ctx, &yen(amount), 7.5, 68.0, d_y);
            draw_line(&layer, 15.0, d_y - 1.5, 103.0, d_y - 1.5);
            d_y -= 8.0;
        }

        // 控除合計
        draw_rect(&layer, 15.0, 78.0, 88.0, 10.0, true, Some((0.90, 0.94, 0.98)));
        draw_text(&layer, &ctx, "所得控除合計 [㉒]:", 9.0, 18.0, 72.0);
        draw_text(&layer, &ctx, &yen(form_b.total_deductions), 9.0, 65.0, 72.0);

        // ページ1 フッター
        draw_text(&layer, &ctx, "所得税及び復興特別所得税の確定申告書B 第一面", 8.0, 75.0, 20.0);
    }

    // ==========================================
    // PAGE 2: 第二面（所得の内訳・控除詳細）
    // ==========================================
    let (page2, layer2) = ctx.doc.add_page(Mm(210.0), Mm(297.0), "Layer 2");
    {
        let layer = ctx.doc.get_page(page2).get_layer(layer2);
        draw_rect(&layer, 12.0, 285.0, 186.0, 270.0, true, None);
        draw_rect(&layer, 12.0, 285.0, 186.0, 14.0, true, Some((0.93, 0.96, 0.94)));
        draw_text(
            &layer,
            &ctx,
            &format!("令和 {} 年分の所得税及び復興特別所得税の確定申告書B 【第二面】", doc.fiscal_year),
            12.0,
            20.0,
            276.0,
        );

        // 所得の内訳
        draw_rect(&layer, 15.0, 265.0, 180.0, 8.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(&layer, &ctx, "○ 所得の内訳（所得の種類、種目・支払者、収入金額、源泉徴収税額）", 9.0, 18.0, 260.0);

        draw_rect(&layer, 15.0, 255.0, 180.0, 8.0, true, Some((0.96, 0.96, 0.96)));
        draw_text(&layer, &ctx, "所得の種類", 8.0, 18.0, 250.0);
        draw_text(&layer, &ctx, "種目・名称", 8.0, 55.0, 250.0);
        draw_text(&layer, &ctx, "収入金額", 8.0, 110.0, 250.0);
        draw_text(&layer, &ctx, "源泉徴収税額", 8.0, 155.0, 250.0);

        draw_text(&layer, &ctx, "事業（営業等）", 8.0, 18.0, 240.0);
        draw_text(&layer, &ctx, if taxpayer.business_name.is_empty() { "個人事業売上" } else { &taxpayer.business_name }, 8.0, 55.0, 240.0);
        draw_text(&layer, &ctx, &yen(blue.sales), 8.0, 110.0, 240.0);
        draw_text(&layer, &ctx, "¥0", 8.0, 155.0, 240.0);
        draw_line(&layer, 15.0, 236.0, 195.0, 236.0);

        // 社会保険料控除等の内訳
        draw_rect(&layer, 15.0, 220.0, 180.0, 8.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(&layer, &ctx, "○ 社会保険料控除等の内訳", 9.0, 18.0, 215.0);

        draw_text(&layer, &ctx, "社会保険の種類: 国民健康保険・国民年金等", 8.0, 18.0, 205.0);
        draw_text(&layer, &ctx, &format!("支払保険料: {}", yen(d.social_insurance)), 8.0, 110.0, 205.0);
        draw_line(&layer, 15.0, 201.0, 195.0, 201.0);

        draw_text(&layer, &ctx, "小規模企業共済: iDeCo・小規模共済等掛金", 8.0, 18.0, 192.0);
        draw_text(&layer, &ctx, &format!("支払掛金: {}", yen(d.small_enterprise_mutual_aid)), 8.0, 110.0, 192.0);
        draw_line(&layer, 15.0, 188.0, 195.0, 188.0);

        // 配偶者や親族に関する事項
        draw_rect(&layer, 15.0, 175.0, 180.0, 8.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(&layer, &ctx, "○ 配偶者・扶養親族等の控除明細", 9.0, 18.0, 170.0);
        draw_text(&layer, &ctx, &format!("配偶者控除額: {}", yen(d.spouse)), 8.0, 18.0, 160.0);
        draw_text(&layer, &ctx, &format!("扶養親族控除額: {}", yen(d.dependents)), 8.0, 110.0, 160.0);

        // 住民税・事業税に関する事項
        draw_rect(&layer, 15.0, 140.0, 180.0, 8.0, true, Some((0.92, 0.95, 0.98)));
        draw_text(&layer, &ctx, "○ 住民税・事業税に関する事項", 9.0, 18.0, 135.0);
        draw_text(&layer, &ctx, "給与・公的年金等以外の所得に係る住民税の徴収方法: 自分で納付（普通徴収）", 8.0, 18.0, 125.0);

        // ページ2 フッター
        draw_text(&layer, &ctx, "所得税及び復興特別所得税の確定申告書B 第二面", 8.0, 75.0, 20.0);
    }

    save_doc(ctx, output_path)
}

fn save_doc(ctx: PdfContext, path: &Path) -> Result<PathBuf, PdfError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    ctx.doc
        .save(&mut writer)
        .map_err(|e| PdfError::Generate(e.to_string()))?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::deductions::TaxDeductions;
    use crate::core::journal::{AccountCode, JournalEntry, TaxpayerInfo};
    use crate::core::tax_calc::BlueSpecialDeductionKind;
    use chrono::NaiveDate;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_export_official_blue_return_and_form_b_pdfs() {
        let mut doc = LedgerDocument::new(2025);
        doc.taxpayer = TaxpayerInfo {
            name: "山田 太郎".into(),
            business_name: "山田デザイン".into(),
            address: "東京都千代田区神田錦町1-1".into(),
            phone: "03-9999-8888".into(),
        };
        doc.special_deduction = BlueSpecialDeductionKind::Yen650k;
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            5_500_000,
            "デザイン受託料",
        ));
        doc.entries.push(JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
            AccountCode::Supplies,
            AccountCode::Bank,
            150_000,
            "タブレットペンタブ購入",
        ));

        let mut deductions = TaxDeductions::default();
        deductions.social_insurance = 400_000;
        doc.deductions = deductions;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("official_pdf_test_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();

        let blue_pdf = dir.join("blue_official.pdf");
        let form_b_pdf = dir.join("form_b_official.pdf");

        let p1 = export_blue_return_official_pdf(&doc, &blue_pdf).unwrap();
        let p2 = export_form_b_official_pdf(&doc, &form_b_pdf).unwrap();

        assert!(p1.is_file());
        assert!(p2.is_file());
        assert!(std::fs::metadata(&p1).unwrap().len() > 1000);
        assert!(std::fs::metadata(&p2).unwrap().len() > 1000);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
