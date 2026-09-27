//! Shared shell layout and sidebar navigation.

use crate::ui::ledger::LedgerPage;
use crate::ui::report::ReportPage;
use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Ledger,
    Report,
}

#[component]
pub fn AppLayout() -> Element {
    let mut route = use_signal(|| Route::Ledger);

    rsx! {
        div { class: "app-shell",
            aside { class: "sidebar",
                div { class: "brand",
                    "確定申告ソフト"
                    small { "仕訳入力 / 帳票" }
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
            }
            main { class: "content",
                match *route.read() {
                    Route::Ledger => rsx! { LedgerPage {} },
                    Route::Report => rsx! { ReportPage {} },
                }
            }
        }
    }
}
