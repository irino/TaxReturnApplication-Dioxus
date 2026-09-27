//! PDF generation for tax forms (coordinate-map driven).

pub mod engine;
pub mod field_map;
pub mod fonts;

pub use engine::{export_form_b_pdf, export_blue_return_pdf, PdfError};
pub use field_map::{FieldMap, TextField};
