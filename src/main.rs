//! Desktop application entry point and window orchestration.

mod ui;

use chrono::{Datelike, Local};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use tax_return_application::core::deductions::TaxDeductions;
use tax_return_application::core::depreciation::FixedAsset;
use tax_return_application::core::journal::{
    AccountDefinition, AllocationRule, JournalEntry, LedgerDocument, TaxpayerInfo,
};
use tax_return_application::core::tax_calc::BlueSpecialDeductionKind;
use tax_return_application::storage::load_year;
use ui::AppLayout;

/// Shared reactive application state (ledger + messages).
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub fiscal_year: i32,
    pub entries: Vec<JournalEntry>,
    pub taxpayer: TaxpayerInfo,
    pub special_deduction: BlueSpecialDeductionKind,
    pub accounts: Vec<AccountDefinition>,
    pub allocation_rules: Vec<AllocationRule>,
    pub fixed_assets: Vec<FixedAsset>,
    pub deductions: TaxDeductions,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
}

impl AppState {
    fn load_initial() -> Self {
        let year = Local::now().year();
        match load_year(year) {
            Ok(doc) => Self::from_document(doc, None),
            Err(e) => Self {
                fiscal_year: year,
                entries: Vec::new(),
                taxpayer: TaxpayerInfo::default(),
                special_deduction: BlueSpecialDeductionKind::default(),
                accounts: AccountDefinition::default_registry(),
                allocation_rules: Vec::new(),
                fixed_assets: Vec::new(),
                deductions: TaxDeductions::default(),
                status_message: None,
                error_message: Some(format!("起動時の読み込みに失敗しました: {e}")),
            },
        }
    }

    pub fn from_document(doc: LedgerDocument, status: Option<String>) -> Self {
        Self {
            fiscal_year: doc.fiscal_year,
            entries: doc.entries,
            taxpayer: doc.taxpayer,
            special_deduction: doc.special_deduction,
            accounts: doc.accounts,
            allocation_rules: doc.allocation_rules,
            fixed_assets: doc.fixed_assets,
            deductions: doc.deductions,
            status_message: status,
            error_message: None,
        }
    }

    pub fn to_document(&self) -> LedgerDocument {
        LedgerDocument {
            version: tax_return_application::core::journal::CURRENT_SCHEMA_VERSION,
            fiscal_year: self.fiscal_year,
            entries: self.entries.clone(),
            taxpayer: self.taxpayer.clone(),
            special_deduction: self.special_deduction,
            accounts: self.accounts.clone(),
            allocation_rules: self.allocation_rules.clone(),
            fixed_assets: self.fixed_assets.clone(),
            deductions: self.deductions.clone(),
        }
    }
}

const STYLES: &str = include_str!("../assets/styles.css");

fn main() {
    #[cfg(target_os = "linux")]
    {
        // WSL2 / Linux環境で画面が真っ白になるWebKitGTK/Mesaの描画問題を回避
        std::env::set_var("GDK_BACKEND", "x11");
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    let window = WindowBuilder::new()
        .with_title("確定申告ソフト")
        .with_inner_size(LogicalSize::new(1180.0, 780.0));

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(window)
                .with_disable_context_menu(false),
        )
        .launch(App);
}

#[component]
fn App() -> Element {
    let state = use_signal(AppState::load_initial);
    use_context_provider(|| state);

    rsx! {
        style { {STYLES} }
        AppLayout {}
    }
}
