//! PDF export engine: draws tax summaries using externalizable field maps.
//!
//! Uses `printpdf` to produce single-binary-friendly filled forms.
//! Layout coordinates live in [`crate::pdf::field_map`] so yearly tweaks
//! do not require rewriting drawing logic.

use crate::core::journal::{format_yen, TaxpayerInfo};
use crate::core::tax_calc::{BlueReturnSummary, FormBSummary};
use crate::pdf::field_map::{FieldMap, TextField};
use crate::pdf::fonts;
use printpdf::{BuiltinFont, IndirectFontRef, Mm, PdfDocument, PdfLayerReference};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Cursor};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("フォントエラー: {0}")]
    Font(String),
    #[error("PDF生成エラー: {0}")]
    Generate(String),
    #[error("入出力エラー: {0}")]
    Io(#[from] io::Error),
}

struct PreparedDoc {
    doc: printpdf::PdfDocumentReference,
    page: printpdf::PdfPageIndex,
    layer: printpdf::PdfLayerIndex,
    font: IndirectFontRef,
    /// True when a Japanese-capable external font was embedded.
    has_cjk_font: bool,
}

fn prepare_document(title: &str, width_mm: f32, height_mm: f32) -> Result<PreparedDoc, PdfError> {
    let (doc, page, layer) = PdfDocument::new(title, Mm(width_mm), Mm(height_mm), "Layer1");

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
            // Fallback: Helvetica (ASCII only). Japanese glyphs will not render correctly.
            let font = doc
                .add_builtin_font(BuiltinFont::Helvetica)
                .map_err(|e| PdfError::Font(e.to_string()))?;
            (font, false)
        }
    };

    Ok(PreparedDoc {
        doc,
        page,
        layer,
        font,
        has_cjk_font,
    })
}

fn draw_text(layer: &PdfLayerReference, prepared: &PreparedDoc, field: &TextField, text: &str) {
    let size = field.font_size;
    let x = Mm(field.x_mm);
    let y = Mm(field.y_mm);
    // When no CJK font is available, strip non-ASCII for safer embedding of numbers/labels.
    let rendered = if prepared.has_cjk_font {
        text.to_string()
    } else {
        ascii_fallback(text)
    };
    layer.use_text(rendered, size, x, y, &prepared.font);
}

fn ascii_fallback(text: &str) -> String {
    let filtered: String = text
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    if filtered.chars().all(|c| c == '?' || c.is_whitespace() || c == ':' || c == '¥' || c == ',' || c.is_ascii_digit() || c.is_ascii_alphabetic() || c == '(' || c == ')' || c == '/' || c == '.' || c == '-') {
        // Prefer a clearer ASCII summary when everything Japanese became '?'
        let digits: String = text.chars().filter(|c| c.is_ascii_digit() || *c == ',' || *c == '-' || *c == '¥' || *c == '.').collect();
        if !digits.is_empty() && filtered.contains('?') {
            return digits;
        }
    }
    filtered
}

fn line_for(field: &TextField, values: &HashMap<&str, String>) -> String {
    let value = values.get(field.id.as_str()).cloned().unwrap_or_default();
    if field.id == "title" || field.id == "note" {
        value
    } else if value.is_empty() {
        format!("{}: ", field.label)
    } else {
        format!("{}: {}", field.label, value)
    }
}

fn paint_map(
    prepared: &PreparedDoc,
    map: &FieldMap,
    values: &HashMap<&str, String>,
) -> Result<(), PdfError> {
    let layer = prepared
        .doc
        .get_page(prepared.page)
        .get_layer(prepared.layer);

    for field in &map.fields {
        let text = line_for(field, values);
        draw_text(&layer, prepared, field, &text);
    }
    Ok(())
}

fn save_doc(prepared: PreparedDoc, path: &Path) -> Result<PathBuf, PdfError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    prepared
        .doc
        .save(&mut writer)
        .map_err(|e| PdfError::Generate(e.to_string()))?;
    Ok(path.to_path_buf())
}

fn yen(v: i64) -> String {
    format!("¥{}", format_yen(v))
}

/// Export 青色申告決算書（損益）概算 PDF.
pub fn export_blue_return_pdf(
    blue: &BlueReturnSummary,
    taxpayer: &TaxpayerInfo,
    output_path: &Path,
) -> Result<PathBuf, PdfError> {
    let map = FieldMap::blue_return_auto();
    let prepared = prepare_document(&map.title, map.page_width_mm, map.page_height_mm)?;

    let mut values: HashMap<&str, String> = HashMap::new();
    values.insert("title", map.title.clone());
    values.insert("year", format!("{}年分", blue.fiscal_year));
    values.insert(
        "name",
        if taxpayer.name.is_empty() {
            "（未入力）".into()
        } else {
            taxpayer.name.clone()
        },
    );
    values.insert(
        "business",
        if taxpayer.business_name.is_empty() {
            "（未入力）".into()
        } else {
            taxpayer.business_name.clone()
        },
    );
    values.insert(
        "address",
        if taxpayer.address.is_empty() {
            "（未入力）".into()
        } else {
            taxpayer.address.clone()
        },
    );
    values.insert("sales", yen(blue.sales));
    values.insert("cogs", yen(blue.cost_of_sales));
    values.insert("gross", yen(blue.gross_profit));
    values.insert("exp_travel", yen(blue.expenses.travel));
    values.insert("exp_comm", yen(blue.expenses.communication));
    values.insert("exp_supplies", yen(blue.expenses.supplies));
    values.insert("exp_rent", yen(blue.expenses.rent));
    values.insert("exp_util", yen(blue.expenses.utilities));
    values.insert("exp_misc", yen(blue.expenses.miscellaneous));
    values.insert("exp_total", yen(blue.total_expenses));
    values.insert("ordinary", yen(blue.ordinary_income_before_deduction));
    values.insert(
        "special",
        format!(
            "{}（{}）",
            yen(blue.special_deduction),
            blue.special_deduction_kind.label_ja()
        ),
    );
    values.insert("income", yen(blue.business_income));
    values.insert(
        "note",
        "※ 本PDFは公式様式の代替となる概算帳票です。提出前に必ず内容を確認してください。".into(),
    );

    paint_map(&prepared, &map, &values)?;
    save_doc(prepared, output_path)
}

/// Export 確定申告書B 概算 PDF.
pub fn export_form_b_pdf(
    form_b: &FormBSummary,
    taxpayer: &TaxpayerInfo,
    output_path: &Path,
) -> Result<PathBuf, PdfError> {
    let map = FieldMap::form_b_auto();
    let prepared = prepare_document(&map.title, map.page_width_mm, map.page_height_mm)?;

    let mut values: HashMap<&str, String> = HashMap::new();
    values.insert("title", map.title.clone());
    values.insert("year", format!("{}年分", form_b.fiscal_year));
    values.insert(
        "name",
        if taxpayer.name.is_empty() {
            "（未入力）".into()
        } else {
            taxpayer.name.clone()
        },
    );
    values.insert(
        "address",
        if taxpayer.address.is_empty() {
            "（未入力）".into()
        } else {
            taxpayer.address.clone()
        },
    );
    values.insert("business_income", yen(form_b.business_income));
    values.insert("total_income", yen(form_b.total_income));
    values.insert("basic_deduction", yen(form_b.basic_deduction));
    values.insert("total_deductions", yen(form_b.total_deductions));
    values.insert("taxable", yen(form_b.taxable_income));
    values.insert("income_tax", yen(form_b.income_tax));
    values.insert("surtax", yen(form_b.reconstruction_surtax));
    values.insert("total_tax", yen(form_b.total_income_tax));
    values.insert(
        "note",
        "※ 概算です。社会保険料控除・扶養控除等は未反映。公式申告には利用しないでください。".into(),
    );

    paint_map(&prepared, &map, &values)?;
    save_doc(prepared, output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::journal::{AccountCode, JournalEntry};
    use crate::core::tax_calc::{build_tax_report, BlueSpecialDeductionKind};
    use chrono::NaiveDate;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn export_pdfs_to_temp() {
        let entries = vec![JournalEntry::new(
            NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
            AccountCode::Bank,
            AccountCode::Sales,
            3_000_000,
            "売上",
        )];
        let report = build_tax_report(&entries, 2025, BlueSpecialDeductionKind::Yen650k);
        let taxpayer = TaxpayerInfo {
            name: "山田 太郎".into(),
            address: "東京都千代田区1-1".into(),
            business_name: "山田商店".into(),
            phone: String::new(),
        };

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tax_pdf_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();

        let blue_path = dir.join("blue.pdf");
        let form_path = dir.join("form_b.pdf");
        let p1 = export_blue_return_pdf(&report.blue, &taxpayer, &blue_path).unwrap();
        let p2 = export_form_b_pdf(&report.form_b, &taxpayer, &form_path).unwrap();
        assert!(p1.is_file());
        assert!(p2.is_file());
        assert!(std::fs::metadata(&p1).unwrap().len() > 100);
        assert!(std::fs::metadata(&p2).unwrap().len() > 100);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
