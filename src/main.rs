//! Desktop application entry point and window orchestration.

mod ui;

use chrono::{Datelike, Local};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use tax_return_application::core::journal::{JournalEntry, LedgerDocument, TaxpayerInfo};
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
        }
    }
}

const STYLES: &str = include_str!("../assets/styles.css");

fn main() {
    let window = WindowBuilder::new()
        .with_title("確定申告ソフト")
        .with_inner_size(LogicalSize::new(1180.0, 780.0));

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(window)
                .with_disable_context_menu(true),
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
