//! Externalizable field coordinates for PDF forms.
//!
//! Units are millimetres from the **bottom-left** of an A4 page (printpdf convention).
//! Annual NTA layout changes can be absorbed by editing these maps (or JSON later)
//! without rewriting drawing code.

use serde::{Deserialize, Serialize};

/// Single text placement on a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextField {
    pub id: String,
    pub label: String,
    /// Millimetres from left edge.
    pub x_mm: f32,
    /// Millimetres from bottom edge.
    pub y_mm: f32,
    pub font_size: f32,
}

/// Page-level field map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMap {
    pub form_id: String,
    pub title: String,
    pub page_width_mm: f32,
    pub page_height_mm: f32,
    pub fields: Vec<TextField>,
}

impl FieldMap {
    pub fn blue_return() -> Self {
        Self {
            form_id: "blue_return_pl".into(),
            title: "所得税青色申告決算書（一般用） 損益計算書（概算）".into(),
            page_width_mm: 210.0,
            page_height_mm: 297.0,
            fields: vec![
                field("title", "帳票タイトル", 20.0, 275.0, 14.0),
                field("year", "対象年", 20.0, 262.0, 11.0),
                field("name", "氏名", 20.0, 250.0, 11.0),
                field("business", "屋号", 20.0, 240.0, 11.0),
                field("address", "住所", 20.0, 230.0, 11.0),
                field("sales", "売上金額", 20.0, 210.0, 11.0),
                field("cogs", "仕入金額", 20.0, 200.0, 11.0),
                field("gross", "差引金額（売上総利益）", 20.0, 190.0, 11.0),
                field("exp_travel", "旅費交通費", 20.0, 170.0, 10.0),
                field("exp_comm", "通信費", 20.0, 160.0, 10.0),
                field("exp_supplies", "消耗品費", 20.0, 150.0, 10.0),
                field("exp_rent", "地代家賃", 20.0, 140.0, 10.0),
                field("exp_util", "水道光熱費", 20.0, 130.0, 10.0),
                field("exp_misc", "雑費", 20.0, 120.0, 10.0),
                field("exp_total", "経費合計", 20.0, 108.0, 11.0),
                field("ordinary", "青色申告特別控除前の所得金額", 20.0, 92.0, 11.0),
                field("special", "青色申告特別控除額", 20.0, 80.0, 11.0),
                field("income", "所得金額", 20.0, 68.0, 12.0),
                field("note", "注記", 20.0, 48.0, 8.0),
            ],
        }
    }

    pub fn form_b() -> Self {
        Self {
            form_id: "form_b".into(),
            title: "所得税及び復興特別所得税の確定申告書B（概算）".into(),
            page_width_mm: 210.0,
            page_height_mm: 297.0,
            fields: vec![
                field("title", "帳票タイトル", 20.0, 275.0, 14.0),
                field("year", "対象年", 20.0, 262.0, 11.0),
                field("name", "氏名", 20.0, 250.0, 11.0),
                field("address", "住所", 20.0, 240.0, 11.0),
                field("business_income", "事業所得", 20.0, 215.0, 11.0),
                field("total_income", "所得金額合計", 20.0, 200.0, 11.0),
                field("basic_deduction", "基礎控除", 20.0, 185.0, 11.0),
                field("total_deductions", "所得控除合計", 20.0, 170.0, 11.0),
                field("taxable", "課税される所得金額", 20.0, 150.0, 11.0),
                field("income_tax", "所得税額", 20.0, 135.0, 11.0),
                field("surtax", "復興特別所得税額", 20.0, 120.0, 11.0),
                field("total_tax", "所得税及び復興特別所得税の額", 20.0, 100.0, 12.0),
                field("note", "注記", 20.0, 70.0, 8.0),
            ],
        }
    }

    /// Serialize map to JSON (for external editing / yearly updates).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Load field map from a JSON file, or fall back to the default constructor.
    pub fn load_from_file_or_fallback<P: AsRef<std::path::Path>>(
        path: P,
        default_fn: fn() -> Self,
    ) -> Self {
        let p = path.as_ref();
        if p.is_file() {
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(map) = serde_json::from_str::<Self>(&content) {
                    return map;
                }
            }
        }
        default_fn()
    }

    /// Blue return field map, preferring `templates/blue_return_fields.json` if available.
    pub fn blue_return_auto() -> Self {
        Self::load_from_file_or_fallback("templates/blue_return_fields.json", Self::blue_return)
    }

    /// Form B field map, preferring `templates/form_b_fields.json` if available.
    pub fn form_b_auto() -> Self {
        Self::load_from_file_or_fallback("templates/form_b_fields.json", Self::form_b)
    }
}

fn field(id: &str, label: &str, x: f32, y: f32, size: f32) -> TextField {
    TextField {
        id: id.into(),
        label: label.into(),
        x_mm: x,
        y_mm: y,
        font_size: size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_maps_roundtrip_json() {
        let map = FieldMap::blue_return();
        let json = map.to_json().unwrap();
        let back: FieldMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.form_id, "blue_return_pl");
        assert!(!back.fields.is_empty());
    }

    #[test]
    fn load_from_file_or_fallback_uses_fallback_on_missing_file() {
        let map = FieldMap::load_from_file_or_fallback("non_existent_file.json", FieldMap::blue_return);
        assert_eq!(map.form_id, "blue_return_pl");
    }

    #[test]
    fn auto_loader_resolves_templates_if_present() {
        let blue = FieldMap::blue_return_auto();
        assert_eq!(blue.form_id, "blue_return_pl");
        let form_b = FieldMap::form_b_auto();
        assert_eq!(form_b.form_id, "form_b");
    }
}

