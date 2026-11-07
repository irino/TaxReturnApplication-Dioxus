//! Year-end closing & settings page (決算・設定).
//!
//! Includes:
//! 1. Custom chart of accounts & sub-account management
//! 2. Home-office expense allocation (家事按分) settings & one-click journal generation
//! 3. Fiscal year balance rollover (年度繰越処理) into the next fiscal year

use crate::AppState;
use dioxus::prelude::*;
use tax_return_application::core::allocation::{
    apply_allocations, calculate_allocations, clear_allocations,
};
use tax_return_application::core::journal::{
    format_yen, AccountCode, AccountNature, AllocationRule,
};
use tax_return_application::core::rollover::{
    calculate_rollover_preview, rollover_to_next_year,
};
use tax_return_application::storage::save_year;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosingTab {
    Accounts,
    Allocation,
    Rollover,
}

#[component]
pub fn ClosingPage() -> Element {
    let mut app = use_context::<Signal<AppState>>();
    let mut active_tab = use_signal(|| ClosingTab::Accounts);

    // カスタム科目追加用フォームステート
    let mut new_account_name = use_signal(|| String::new());
    let mut new_account_nature = use_signal(|| AccountNature::Expense);
    let mut account_error = use_signal(|| Option::<String>::None);

    // 補助科目追加用フォームステート (対象科目のコード文字列, 入力値)
    let mut target_sub_account_code = use_signal(|| Option::<String>::None);
    let mut new_sub_name = use_signal(|| String::new());

    // 家事按分ルール追加用フォームステート
    let mut alloc_account = use_signal(|| AccountCode::Rent);
    let mut alloc_ratio = use_signal(|| 50u8);
    let mut alloc_sub = use_signal(|| String::new());
    let mut alloc_error = use_signal(|| Option::<String>::None);

    let state = app.read().clone();
    let fiscal_year = state.fiscal_year;
    let status = state.status_message.clone();
    let global_error = state.error_message.clone();

    let accounts_list = state.accounts.clone();
    let doc_snapshot = state.to_document();
    let alloc_calcs = calculate_allocations(&doc_snapshot);
    let rollover_preview = calculate_rollover_preview(&doc_snapshot);

    rsx! {
        h1 { class: "page-title", "{fiscal_year}年 決算・設定" }

        if let Some(msg) = status {
            div { class: "status-banner", "{msg}" }
        }
        if let Some(msg) = global_error {
            div { class: "error-banner", "{msg}" }
        }

        // タブ切り替えバー
        div { class: "tab-bar",
            button {
                class: if *active_tab.read() == ClosingTab::Accounts { "tab-btn active" } else { "tab-btn" },
                onclick: move |_| active_tab.set(ClosingTab::Accounts),
                "勘定科目・補助科目設定"
            }
            button {
                class: if *active_tab.read() == ClosingTab::Allocation { "tab-btn active" } else { "tab-btn" },
                onclick: move |_| active_tab.set(ClosingTab::Allocation),
                "家事按分設定 & 実行"
            }
            button {
                class: if *active_tab.read() == ClosingTab::Rollover { "tab-btn active" } else { "tab-btn" },
                onclick: move |_| active_tab.set(ClosingTab::Rollover),
                "年度繰越処理"
            }
        }

        match *active_tab.read() {
            ClosingTab::Accounts => rsx! {
                // ---- 勘定科目・補助科目設定 ----
                div { class: "card",
                    h2 { "勘定科目の追加（カスタム科目）" }
                    p { class: "muted", "業種や事業内容に合わせて自由な勘定科目を追加できます。" }

                    if let Some(err) = account_error.read().clone() {
                        div { class: "error-banner", "{err}" }
                    }

                    div { class: "form-grid",
                        div { class: "field",
                            label { "勘定科目名" }
                            input {
                                r#type: "text",
                                placeholder: "例: 広告宣伝費、クラウド利用料",
                                value: "{new_account_name}",
                                oninput: move |evt| new_account_name.set(evt.value()),
                            }
                        }
                        div { class: "field",
                            label { "分類" }
                            select {
                                value: match *new_account_nature.read() {
                                    AccountNature::Asset => "asset",
                                    AccountNature::Liability => "liability",
                                    AccountNature::Equity => "equity",
                                    AccountNature::Income => "income",
                                    AccountNature::Expense => "expense",
                                },
                                onchange: move |evt| {
                                    let n = match evt.value().as_str() {
                                        "asset" => AccountNature::Asset,
                                        "liability" => AccountNature::Liability,
                                        "equity" => AccountNature::Equity,
                                        "income" => AccountNature::Income,
                                        _ => AccountNature::Expense,
                                    };
                                    new_account_nature.set(n);
                                },
                                option { value: "expense", "費用（経費）" }
                                option { value: "income", "収益（売上・雑収入）" }
                                option { value: "asset", "資産" }
                                option { value: "liability", "負債" }
                                option { value: "equity", "純資産" }
                            }
                        }
                    }
                    div { class: "actions", style: "margin-top: 0.8rem;",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                account_error.set(None);
                                let name = new_account_name.read().trim().to_string();
                                let nature = *new_account_nature.read();
                                let mut doc = app.read().to_document();
                                match doc.add_custom_account(&name, nature) {
                                    Ok(_) => {
                                        if let Err(e) = save_year(&doc) {
                                            app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                        } else {
                                            *app.write() = AppState::from_document(doc, Some(format!("科目「{name}」を追加しました")));
                                            new_account_name.set(String::new());
                                        }
                                    }
                                    Err(err) => account_error.set(Some(err)),
                                }
                            },
                            "科目を追加"
                        }
                    }
                }

                div { class: "card",
                    h2 { "勘定科目・補助科目一覧" }
                    p { class: "muted", "各科目に紐づく補助科目（銀行名、取引先名など）を登録・管理できます。" }

                    table { class: "ledger",
                        thead {
                            tr {
                                th { "分類" }
                                th { "科目名" }
                                th { "種類" }
                                th { "登録済み補助科目" }
                                th { style: "text-align: right;", "操作" }
                            }
                        }
                        tbody {
                            for acc in accounts_list.into_iter() {
                                tr {
                                    key: "{acc.name}",
                                    td {
                                        span {
                                            class: match acc.nature {
                                                AccountNature::Asset => "badge badge-asset",
                                                AccountNature::Liability => "badge badge-liability",
                                                AccountNature::Equity => "badge badge-equity",
                                                AccountNature::Income => "badge badge-income",
                                                AccountNature::Expense => "badge badge-expense",
                                            },
                                            "{acc.nature.label_ja()}"
                                        }
                                    }
                                    td { strong { "{acc.name}" } }
                                    td {
                                        if acc.is_custom {
                                            span { class: "badge badge-custom", "カスタム" }
                                        } else {
                                            span { class: "muted", "標準" }
                                        }
                                    }
                                    td {
                                        div { style: "display: flex; flex-wrap: wrap; gap: 0.35rem; align-items: center;",
                                            for sub in acc.sub_accounts.iter() {
                                                {
                                                    let s = sub.clone();
                                                    let code = acc.code.clone();
                                                    rsx! {
                                                        span { class: "sub-badge-interactive",
                                                            "{s}"
                                                            button {
                                                                class: "sub-delete-btn",
                                                                title: "補助科目を削除",
                                                                onclick: move |_| {
                                                                    let mut doc = app.read().to_document();
                                                                    if let Ok(_) = doc.remove_sub_account(&code, &s) {
                                                                        let _ = save_year(&doc);
                                                                        *app.write() = AppState::from_document(doc, Some(format!("補助科目「{s}」を削除しました")));
                                                                    }
                                                                },
                                                                "×"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            {
                                                let code_str = acc.name.clone();
                                                let code = acc.code.clone();
                                                let is_adding = *target_sub_account_code.read() == Some(code_str.clone());
                                                rsx! {
                                                    if is_adding {
                                                        span { style: "display: inline-flex; align-items: center; gap: 0.3rem;",
                                                            input {
                                                                r#type: "text",
                                                                style: "font-size: 0.78rem; padding: 0.2rem 0.4rem; width: 120px;",
                                                                placeholder: "補助科目名",
                                                                value: "{new_sub_name}",
                                                                oninput: move |evt| new_sub_name.set(evt.value()),
                                                            }
                                                            button {
                                                                class: "btn btn-primary btn-sm",
                                                                style: "padding: 0.2rem 0.5rem; font-size: 0.75rem;",
                                                                onclick: move |_| {
                                                                    let name = new_sub_name.read().trim().to_string();
                                                                    if !name.is_empty() {
                                                                        let mut doc = app.read().to_document();
                                                                        if let Ok(_) = doc.add_sub_account(&code, &name) {
                                                                            let _ = save_year(&doc);
                                                                            *app.write() = AppState::from_document(doc, Some(format!("補助科目「{name}」を追加しました")));
                                                                            new_sub_name.set(String::new());
                                                                            target_sub_account_code.set(None);
                                                                        }
                                                                    }
                                                                },
                                                                "追加"
                                                            }
                                                            button {
                                                                class: "btn btn-secondary btn-sm",
                                                                style: "padding: 0.2rem 0.4rem; font-size: 0.75rem;",
                                                                onclick: move |_| {
                                                                    target_sub_account_code.set(None);
                                                                    new_sub_name.set(String::new());
                                                                },
                                                                "取消"
                                                            }
                                                        }
                                                    } else {
                                                        button {
                                                            class: "btn btn-secondary btn-sm",
                                                            style: "padding: 0.15rem 0.45rem; font-size: 0.75rem;",
                                                            onclick: move |_| {
                                                                target_sub_account_code.set(Some(code_str.clone()));
                                                                new_sub_name.set(String::new());
                                                            },
                                                            "＋補助科目"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    td { style: "text-align: right;",
                                        if acc.is_custom {
                                            {
                                                let code = acc.code.clone();
                                                let name = acc.name.clone();
                                                rsx! {
                                                    button {
                                                        class: "btn btn-danger btn-sm",
                                                        onclick: move |_| {
                                                            let mut doc = app.read().to_document();
                                                            match doc.remove_custom_account(&code) {
                                                                Ok(_) => {
                                                                    let _ = save_year(&doc);
                                                                    *app.write() = AppState::from_document(doc, Some(format!("科目「{name}」を削除しました")));
                                                                }
                                                                Err(e) => {
                                                                    app.write().error_message = Some(e);
                                                                }
                                                            }
                                                        },
                                                        "削除"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },

            ClosingTab::Allocation => rsx! {
                // ---- 家事按分設定 & 実行 ----
                div { class: "card",
                    h2 { "家事按分ルールの追加" }
                    p { class: "muted",
                        "自宅兼事務所の家賃、電気代、通信費など、事業とプライベートが混在する経費の事業割合（%）を設定します。"
                        br {}
                        "年末に事業割合に基づき、個人負担分を「事業主貸」へ振り替える仕訳を自動生成します。"
                    }

                    if let Some(err) = alloc_error.read().clone() {
                        div { class: "error-banner", "{err}" }
                    }

                    div { class: "form-grid",
                        div { class: "field",
                            label { "対象経費科目" }
                            select {
                                value: "{alloc_account.read().label_ja()}",
                                onchange: move |evt| {
                                    let state = app.read();
                                    if let Some(acc) = state.accounts.iter().find(|a| a.name == evt.value()) {
                                        alloc_account.set(acc.code.clone());
                                    }
                                },
                                for acc in app.read().accounts.iter().filter(|a| a.nature == AccountNature::Expense) {
                                    option {
                                        value: "{acc.name}",
                                        selected: alloc_account.read().label_ja() == acc.name,
                                        "{acc.name}"
                                    }
                                }
                            }
                        }
                        div { class: "field",
                            label { "事業利用割合: {alloc_ratio}% （個人負担: {100 - *alloc_ratio.read()}%）" }
                            input {
                                r#type: "range",
                                min: "0",
                                max: "100",
                                step: "5",
                                value: "{alloc_ratio}",
                                oninput: move |evt| {
                                    if let Ok(v) = evt.value().parse::<u8>() {
                                        alloc_ratio.set(v.min(100));
                                    }
                                },
                            }
                        }
                        div { class: "field",
                            label { "特定の補助科目に限定（任意）" }
                            select {
                                value: "{alloc_sub}",
                                onchange: move |evt| alloc_sub.set(evt.value()),
                                option { value: "", "科目の全体を按分" }
                                {
                                    let cur_code = alloc_account.read().clone();
                                    let subs = app.read().accounts.iter().find(|a| a.code == cur_code).map(|a| a.sub_accounts.clone()).unwrap_or_default();
                                    rsx! {
                                        for s in subs {
                                            option { value: "{s}", "{s}" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "actions", style: "margin-top: 0.8rem;",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                alloc_error.set(None);
                                let mut doc = app.read().to_document();
                                let target_code = alloc_account.read().clone();
                                let sub = if alloc_sub.read().trim().is_empty() { None } else { Some(alloc_sub.read().trim().to_string()) };
                                let ratio = *alloc_ratio.read();

                                // 既存重複チェック
                                doc.allocation_rules.retain(|r| !(r.account == target_code && r.sub_account == sub));
                                doc.allocation_rules.push(AllocationRule {
                                    account: target_code.clone(),
                                    business_ratio_percent: ratio,
                                    sub_account: sub,
                                });

                                if let Err(e) = save_year(&doc) {
                                    app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                } else {
                                    *app.write() = AppState::from_document(doc, Some(format!("「{}」の按分ルールを設定しました", target_code.label_ja())));
                                }
                            },
                            "按分ルールを保存"
                        }
                    }
                }

                div { class: "card",
                    div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem;",
                        h2 { style: "margin: 0;", "家事按分の試算プレビュー & 仕訳反映" }
                        div { class: "actions",
                            button {
                                class: "btn btn-primary",
                                disabled: alloc_calcs.is_empty(),
                                onclick: move |_| {
                                    let mut doc = app.read().to_document();
                                    let count = apply_allocations(&mut doc);
                                    if let Err(e) = save_year(&doc) {
                                        app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                    } else {
                                        *app.write() = AppState::from_document(
                                            doc,
                                            Some(format!("12月31日付で {count} 件の家事按分仕訳を生成・反映しました")),
                                        );
                                    }
                                },
                                "⚡ 家事按分仕訳を一括生成・反映"
                            }
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| {
                                    let mut doc = app.read().to_document();
                                    let count = clear_allocations(&mut doc);
                                    if let Err(e) = save_year(&doc) {
                                        app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                    } else {
                                        *app.write() = AppState::from_document(
                                            doc,
                                            Some(format!("{count} 件の家事按分仕訳を削除しました")),
                                        );
                                    }
                                },
                                "按分仕訳をクリア"
                            }
                        }
                    }

                    if alloc_calcs.is_empty() {
                        div { class: "empty", "按分ルールが設定されていません。上のフォームから登録してください。" }
                    } else {
                        table { class: "ledger", style: "margin-top: 1rem;",
                            thead {
                                tr {
                                    th { "科目" }
                                    th { "補助科目" }
                                    th { "年間発生総額" }
                                    th { "事業割合" }
                                    th { "事業経費（残額）" }
                                    th { "個人負担（振替額）" }
                                    th { style: "text-align: right;", "操作" }
                                }
                            }
                            tbody {
                                for calc in alloc_calcs.iter() {
                                    tr {
                                        key: "{calc.account_label}_{calc.sub_account.as_deref().unwrap_or_default()}",
                                        td { strong { "{calc.account_label}" } }
                                        td { "{calc.sub_account.as_deref().unwrap_or(\"-\")}" }
                                        td { class: "amount", "¥{format_yen(calc.total_expense)}" }
                                        td { "{calc.business_ratio_percent}%" }
                                        td { class: "amount", style: "color: var(--success); font-weight: 600;", "¥{format_yen(calc.business_expense)}" }
                                        td { class: "amount", style: "color: var(--danger); font-weight: 600;", "¥{format_yen(calc.personal_expense)}" }
                                        td { style: "text-align: right;",
                                            {
                                                let acc = calc.account.clone();
                                                let sub = calc.sub_account.clone();
                                                rsx! {
                                                    button {
                                                        class: "btn btn-danger btn-sm",
                                                        onclick: move |_| {
                                                            let mut doc = app.read().to_document();
                                                            doc.allocation_rules.retain(|r| !(r.account == acc && r.sub_account == sub));
                                                            let _ = save_year(&doc);
                                                            *app.write() = AppState::from_document(doc, Some("按分ルールを削除しました".into()));
                                                        },
                                                        "ルール削除"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },

            ClosingTab::Rollover => rsx! {
                // ---- 年度繰越処理 ----
                div { class: "card",
                    h2 { "{fiscal_year}年 から {fiscal_year + 1}年 への年度繰越" }
                    p { class: "muted",
                        "青色申告決算書の貸借対照表（B/S）期末残高（現金、普通預金、売掛金、買掛金等）を翌年の期首残高として引き継ぎます。"
                        br {}
                        "当期純利益（所得金額）、事業主貸、事業主借は翌期の「元入金」に合算・清算され、翌期首の開始仕訳が自動生成されます。"
                    }

                    div { class: "report-grid", style: "margin-top: 1rem;",
                        div { class: "stat",
                            div { class: "label", "資産残高合計（期末）" }
                            div { class: "value", "¥{format_yen(rollover_preview.total_assets)}" }
                        }
                        div { class: "stat",
                            div { class: "label", "負債残高合計（期末）" }
                            div { class: "value", "¥{format_yen(rollover_preview.total_liabilities)}" }
                        }
                        div { class: "stat",
                            div { class: "label", "前期末 元入金" }
                            div { class: "value", "¥{format_yen(rollover_preview.prev_capital)}" }
                        }
                        div { class: "stat",
                            div { class: "label", "当期純所得（控除前）" }
                            div { class: "value", "¥{format_yen(rollover_preview.net_income)}" }
                        }
                        div { class: "stat",
                            div { class: "label", "事業主借" }
                            div { class: "value", "¥{format_yen(rollover_preview.owner_contribution)}" }
                        }
                        div { class: "stat",
                            div { class: "label", "事業主貸" }
                            div { class: "value", "¥{format_yen(rollover_preview.drawings)}" }
                        }
                        div { class: "stat wide", style: "border-left: 4px solid var(--accent); background: var(--bg-hover);",
                            div { class: "label", "👉 翌期（{fiscal_year + 1}年）開始時 元入金" }
                            div { class: "value", style: "color: var(--accent); font-size: 1.5rem;",
                                "¥{format_yen(rollover_preview.next_capital)}"
                            }
                            small { class: "muted", "計算式: 前期末元入金 + 当期純所得 + 事業主借 - 事業主貸" }
                        }
                    }

                    div { style: "margin-top: 1.5rem;",
                        h3 { style: "font-size: 1rem; margin-bottom: 0.5rem;", "繰り越される資産科目残高" }
                        if rollover_preview.asset_balances.is_empty() {
                            div { class: "empty", "繰越対象の資産残高はありません。" }
                        } else {
                            table { class: "ledger",
                                thead {
                                    tr {
                                        th { "資産科目" }
                                        th { "翌期首残高" }
                                    }
                                }
                                tbody {
                                    for (name, amt) in rollover_preview.asset_balances.iter() {
                                        tr {
                                            td { "{name}" }
                                            td { class: "amount", "¥{format_yen(*amt)}" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { style: "margin-top: 1.5rem;",
                        h3 { style: "font-size: 1rem; margin-bottom: 0.5rem;", "繰り越される負債科目残高" }
                        if rollover_preview.liability_balances.is_empty() {
                            div { class: "empty", "繰越対象の負債残高はありません。" }
                        } else {
                            table { class: "ledger",
                                thead {
                                    tr {
                                        th { "負債科目" }
                                        th { "翌期首残高" }
                                    }
                                }
                                tbody {
                                    for (name, amt) in rollover_preview.liability_balances.iter() {
                                        tr {
                                            td { "{name}" }
                                            td { class: "amount", "¥{format_yen(*amt)}" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "actions", style: "margin-top: 1.5rem; padding-top: 1rem; border-top: 1px solid var(--border);",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let prev = app.read().to_document();
                                let next_doc = rollover_to_next_year(&prev);
                                let next_year = next_doc.fiscal_year;
                                match save_year(&next_doc) {
                                    Ok(_) => {
                                        *app.write() = AppState::from_document(
                                            next_doc,
                                            Some(format!("{next_year}年度を作成し、期首残高とマスタ設定を引き継ぎました")),
                                        );
                                    }
                                    Err(e) => {
                                        app.write().error_message = Some(format!("翌年度の保存に失敗しました: {e}"));
                                    }
                                }
                            },
                            "🚀 {fiscal_year + 1}年度のデータを作成して繰越を実行"
                        }
                    }
                }
            }
        }
    }
}
