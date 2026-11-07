//! Pure tax and accounting logic. No UI dependencies.

pub mod allocation;
pub mod csv_import;
pub mod journal;
pub mod rollover;
pub mod tax_calc;

pub use allocation::{
    apply_allocations, calculate_allocations, clear_allocations, generate_allocation_entries,
    is_allocation_entry, AllocationCalculation,
};
pub use csv_import::{
    decode_csv_bytes, parse_flexible_date, parse_legacy_csv, parse_legacy_csv_str,
    CsvImportReport, CsvRowError,
};
pub use journal::{
    validate_entry, AccountCode, AccountDefinition, AccountNature, AllocationRule,
    JournalEntry, JournalError, LedgerDocument, TaxpayerInfo, CURRENT_SCHEMA_VERSION,
};
pub use rollover::{
    calculate_rollover_preview, is_rollover_entry, rollover_to_next_year, RolloverPreview,
};
pub use tax_calc::{
    aggregate_account_totals, aggregate_blue_return, aggregate_form_b, build_tax_report,
    BlueReturnSummary, BlueSpecialDeductionKind, FormBSummary, TaxReportBundle,
};
