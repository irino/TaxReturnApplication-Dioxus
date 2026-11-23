//! Shared shell layout and sidebar navigation.

use crate::ui::analytics::AnalyticsPage;
use crate::ui::closing::ClosingPage;
use crate::ui::ledger::LedgerPage;
use crate::ui::report::ReportPage;
use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Ledger,
    Report,
    Closing,
    Analytics,
}

#[component]
pub fn AppLayout() -> Element {
    let mut route = use_signal(|| Route::Ledger);

    rsx! {
        div { class: "app-shell",
            aside { class: "sidebar",
                div { class: "brand",
                    "確定申告ソフト"
                    small { "仕訳 / 帳票 / 決算 / 分析" }
                }
                button {
                    class: if *route.read() == Route::Ledger { "nav-btn active" } else { "nav-btn" },
                    onclick: move |_| route.set(Route::Ledger),
                    "仕訳"
                }
                button {
                    class: if *route.read() == Route::Report { "nav-btn active" } else { "nav-btn" },
                    onclick: move |_| route.set(Route::Report),
                    "帳票"
                }
                button {
                    class: if *route.read() == Route::Closing { "nav-btn active" } else { "nav-btn" },
                    onclick: move |_| route.set(Route::Closing),
                    "決算・設定"
                }
                button {
                    class: if *route.read() == Route::Analytics { "nav-btn active" } else { "nav-btn" },
                    onclick: move |_| route.set(Route::Analytics),
                    "📊 経営分析"
                }
            }
            main { class: "content",
                match *route.read() {
                    Route::Ledger => rsx! { LedgerPage {} },
                    Route::Report => rsx! { ReportPage {} },
                    Route::Closing => rsx! { ClosingPage {} },
                    Route::Analytics => rsx! { AnalyticsPage {} },
                }
            }
        }
    }
}
