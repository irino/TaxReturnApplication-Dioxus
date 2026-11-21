//! Pure tax and accounting logic. No UI dependencies.

pub mod allocation;
pub mod auto_rule;
pub mod beancount;
pub mod compound;
pub mod csv_import;
pub mod deductions;
pub mod depreciation;
pub mod etax;
pub mod journal;
pub mod rollover;
pub mod tax_calc;
pub mod universal_csv;

pub use allocation::{
    apply_allocations, calculate_allocations, clear_allocations, generate_allocation_entries,
    is_allocation_entry, AllocationCalculation,
};
pub use auto_rule::{default_auto_rules, match_auto_rule, AutoBookingRule, RuleMatchType};
pub use beancount::{account_to_beancount_name, beancount_name_to_account, export_to_beancount, import_from_beancount};
pub use compound::{CompoundError, CompoundSplit, CompoundTransaction};
pub use csv_import::{
    decode_csv_bytes, parse_flexible_date, parse_legacy_csv, parse_legacy_csv_str,
    CsvImportReport, CsvRowError,
};
pub use deductions::{
    compute_all_deductions, CalculatedDeductions, SpouseDeductionType, TaxDeductions,
    WidowOrSingleParentType,
};
pub use depreciation::{
    apply_depreciation_entries, calculate_all_depreciations, calculate_asset_depreciation,
    clear_depreciation_entries, generate_depreciation_entries, is_depreciation_entry,
    straight_line_depreciation_rate, DepreciationCalculation, DepreciationMethod, FixedAsset,
};
pub use etax::{escape_xml, generate_etax_xml, validate_etax_xml};
pub use journal::{
    validate_entry, AccountCode, AccountDefinition, AccountNature, AllocationRule,
    JournalEntry, JournalError, LedgerDocument, TaxpayerInfo, CURRENT_SCHEMA_VERSION,
};
pub use rollover::{
    calculate_rollover_preview, is_rollover_entry, rollover_to_next_year, RolloverPreview,
};
pub use tax_calc::{
    aggregate_account_totals, aggregate_balance_sheet_breakdown, aggregate_blue_return,
    aggregate_form_b, aggregate_form_b_with_deductions, aggregate_monthly_breakdown,
    build_tax_report, build_tax_report_with_deductions, BalanceSheetBreakdown, BalanceSheetItem,
    BlueReturnSummary, BlueSpecialDeductionKind, FormBSummary, MonthlyBreakdown, TaxReportBundle,
};
pub use universal_csv::{
    parse_universal_csv, parse_universal_csv_str, CsvAmountMode, CsvMappingConfig,
    UniversalCsvReport, UniversalCsvRow,
};
