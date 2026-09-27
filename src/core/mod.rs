//! Pure tax and accounting logic. No UI dependencies.

pub mod journal;
pub mod tax_calc;

pub use journal::{
    validate_entry, AccountCode, AccountNature, JournalEntry, JournalError, LedgerDocument,
    TaxpayerInfo, CURRENT_SCHEMA_VERSION,
};
pub use tax_calc::{
    aggregate_account_totals, aggregate_blue_return, aggregate_form_b, build_tax_report,
    BlueReturnSummary, BlueSpecialDeductionKind, FormBSummary, TaxReportBundle,
};
