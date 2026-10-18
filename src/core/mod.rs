//! Pure tax and accounting logic. No UI dependencies.

pub mod csv_import;
pub mod journal;
pub mod tax_calc;

pub use csv_import::{
    decode_csv_bytes, parse_flexible_date, parse_legacy_csv, parse_legacy_csv_str,
    CsvImportReport, CsvRowError,
};

pub use journal::{
    validate_entry, AccountCode, AccountNature, JournalEntry, JournalError, LedgerDocument,
    TaxpayerInfo, CURRENT_SCHEMA_VERSION,
};
pub use tax_calc::{
    aggregate_account_totals, aggregate_blue_return, aggregate_form_b, build_tax_report,
    BlueReturnSummary, BlueSpecialDeductionKind, FormBSummary, TaxReportBundle,
};
