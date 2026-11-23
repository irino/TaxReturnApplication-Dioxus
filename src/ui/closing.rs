//! Year-end closing & settings page (決算・設定).
//!
//! Includes:
//! 1. Custom chart of accounts & sub-account management
//! 2. Home-office expense allocation (家事按分) settings & one-click journal generation
//! 3. Fiscal year balance rollover (年度繰越処理) into the next fiscal year

use crate::AppState;
use chrono::NaiveDate;
use dioxus::prelude::*;
use tax_return_application::core::allocation::{
    apply_allocations, calculate_allocations, clear_allocations,
};
use tax_return_application::core::auto_rule::{default_auto_rules, AutoBookingRule, RuleMatchType};
use tax_return_application::core::beancount::{export_to_beancount, import_from_beancount};
use tax_return_application::core::depreciation::{
    apply_depreciation_entries, calculate_all_depreciations, clear_depreciation_entries,
    DepreciationMethod, FixedAsset,
};
use tax_return_application::core::journal::{
    format_yen, AccountCode, AccountNature, AllocationRule,
};
use tax_return_application::core::rollover::{
    calculate_rollover_preview, rollover_to_next_year,
};
use tax_return_application::storage::{
    auto_migrate_if_needed, export_gnucash_database, exports_dir, get_sqlite_stats,
    open_sqlite_connection, save_year, sqlite_db_path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosingTab {
    Accounts,
    Depreciation,
    Allocation,
    Rollover,
    Storage,
    AutoRules,
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

    // 固定資産追加用フォームステート
    let mut asset_name = use_signal(|| String::new());
    let mut asset_date_str = use_signal(|| String::new());
    let mut asset_cost_str = use_signal(|| String::new());
    let mut asset_years = use_signal(|| 4u32);
    let mut asset_method = use_signal(|| DepreciationMethod::StraightLine);
    let mut asset_ratio = use_signal(|| 100u8);
    let mut asset_account = use_signal(|| AccountCode::ToolsAndEquipment);
    let mut asset_memo = use_signal(|| String::new());
    let mut asset_error = use_signal(|| Option::<String>::None);

    // 家事按分ルール追加用フォームステート
    let mut alloc_account = use_signal(|| AccountCode::Rent);
    let mut alloc_ratio = use_signal(|| 50u8);
    let mut alloc_sub = use_signal(|| String::new());
    let mut alloc_error = use_signal(|| Option::<String>::None);

    // Beancount入出力用ステート
    let mut beancount_input = use_signal(|| String::new());
    let mut beancount_preview = use_signal(|| String::new());

    // 自動仕訳ルール用ステート
    let mut new_rule_name = use_signal(|| String::new());
    let mut new_rule_pattern = use_signal(|| String::new());
    let mut new_rule_match_type = use_signal(|| RuleMatchType::Contains);
    let mut new_rule_account = use_signal(|| AccountCode::Supplies);
    let mut new_rule_sub = use_signal(|| String::new());
    let mut new_rule_priority = use_signal(|| 0i32);
    let mut rule_error = use_signal(|| Option::<String>::None);

    let state = app.read().clone();
    let fiscal_year = state.fiscal_year;
    let status = state.status_message.clone();
    let global_error = state.error_message.clone();

    let accounts_list = state.accounts.clone();
    let auto_rules_list = state.auto_rules.clone();
    let doc_snapshot = state.to_document();
    let dep_calcs = calculate_all_depreciations(&doc_snapshot);
    let total_dep_cost: i64 = dep_calcs.iter().map(|c| c.acquisition_cost).sum();
    let total_dep_opening: i64 = dep_calcs.iter().map(|c| c.opening_book_value).sum();
    let total_dep_ordinary: i64 = dep_calcs.iter().map(|c| c.ordinary_depreciation).sum();
    let total_dep_expense: i64 = dep_calcs.iter().map(|c| c.business_expense).sum();
    let total_dep_closing: i64 = dep_calcs.iter().map(|c| c.closing_book_value).sum();
    let alloc_calcs = calculate_allocations(&doc_snapshot);
    let rollover_preview = calculate_rollover_preview(&doc_snapshot);

    let sqlite_path = sqlite_db_path().unwrap_or_default();
    let sqlite_stats = open_sqlite_connection(&sqlite_path)
        .ok()
        .and_then(|c| get_sqlite_stats(&sqlite_path, &c).ok());

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
                class: if *active_tab.read() == ClosingTab::Depreciation { "tab-btn active" } else { "tab-btn" },
                onclick: move |_| active_tab.set(ClosingTab::Depreciation),
                "固定資産台帳・減価償却"
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
            button {
                class: if *active_tab.read() == ClosingTab::Storage { "tab-btn active" } else { "tab-btn" },
                onclick: move |_| active_tab.set(ClosingTab::Storage),
                "データ保存 & 相互運用"
            }
            button {
                class: if *active_tab.read() == ClosingTab::AutoRules { "tab-btn active" } else { "tab-btn" },
                onclick: move |_| active_tab.set(ClosingTab::AutoRules),
                "自動仕訳ルール設定"
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

            ClosingTab::Depreciation => rsx! {
                // ---- 固定資産台帳 & 減価償却 ----
                div { class: "card",
                    h2 { "固定資産の新規登録" }
                    p { class: "muted",
                        "PC、車両、備品、ソフトウェアなどの固定資産を登録します。耐用年数や償却方法（定額法・一括償却3年・少額特例30万円）に応じて年末減価償却費が自動計算されます。"
                    }

                    if let Some(err) = asset_error.read().clone() {
                        div { class: "error-banner", "{err}" }
                    }

                    div { class: "form-grid",
                        div { class: "field",
                            label { "資産の名称" }
                            input {
                                r#type: "text",
                                placeholder: "例: MacBook Pro M3, 社用車",
                                value: "{asset_name}",
                                oninput: move |evt| asset_name.set(evt.value()),
                            }
                        }
                        div { class: "field",
                            label { "取得年月日" }
                            input {
                                r#type: "date",
                                value: "{asset_date_str}",
                                oninput: move |evt| asset_date_str.set(evt.value()),
                            }
                        }
                        div { class: "field",
                            label { "取得価額 (円)" }
                            input {
                                r#type: "number",
                                min: "1",
                                placeholder: "例: 250000",
                                value: "{asset_cost_str}",
                                oninput: move |evt| asset_cost_str.set(evt.value()),
                            }
                        }
                        div { class: "field",
                            label { "償却方法" }
                            select {
                                value: match *asset_method.read() {
                                    DepreciationMethod::StraightLine => "straight_line",
                                    DepreciationMethod::LumpSum3Year => "lump_sum_3year",
                                    DepreciationMethod::ImmediateSmallAsset => "immediate_small_asset",
                                },
                                onchange: move |evt| {
                                    let m = match evt.value().as_str() {
                                        "lump_sum_3year" => DepreciationMethod::LumpSum3Year,
                                        "immediate_small_asset" => DepreciationMethod::ImmediateSmallAsset,
                                        _ => DepreciationMethod::StraightLine,
                                    };
                                    asset_method.set(m);
                                },
                                option { value: "straight_line", "定額法（原則）" }
                                option { value: "lump_sum_3year", "一括償却（3年均等・20万円未満）" }
                                option { value: "immediate_small_asset", "少額減価償却特例（即時償却・30万円未満）" }
                            }
                        }
                        div { class: "field",
                            label { "耐用年数 (年)" }
                            input {
                                r#type: "number",
                                min: "2",
                                max: "50",
                                value: "{asset_years}",
                                oninput: move |evt| {
                                    if let Ok(y) = evt.value().parse::<u32>() {
                                        asset_years.set(y);
                                    }
                                },
                            }
                        }
                        div { class: "field",
                            label { "事業専用割合 (%)" }
                            input {
                                r#type: "number",
                                min: "1",
                                max: "100",
                                value: "{asset_ratio}",
                                oninput: move |evt| {
                                    if let Ok(r) = evt.value().parse::<u8>() {
                                        asset_ratio.set(r.clamp(1, 100));
                                    }
                                },
                            }
                        }
                        div { class: "field",
                            label { "資産科目" }
                            select {
                                onchange: move |evt| {
                                    let code = match evt.value().as_str() {
                                        "vehicles" => AccountCode::Vehicles,
                                        "buildings" => AccountCode::Buildings,
                                        "software" => AccountCode::Software,
                                        _ => AccountCode::ToolsAndEquipment,
                                    };
                                    asset_account.set(code);
                                },
                                option { value: "tools_and_equipment", "工具器具備品" }
                                option { value: "vehicles", "車両運搬具" }
                                option { value: "buildings", "建物" }
                                option { value: "software", "ソフトウェア" }
                            }
                        }
                        div { class: "field",
                            label { "メモ・用途" }
                            input {
                                r#type: "text",
                                placeholder: "任意メモ",
                                value: "{asset_memo}",
                                oninput: move |evt| asset_memo.set(evt.value()),
                            }
                        }
                    }

                    div { class: "actions", style: "margin-top: 1rem;",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                asset_error.set(None);
                                let name = asset_name.read().trim().to_string();
                                if name.is_empty() {
                                    asset_error.set(Some("資産の名称を入力してください".into()));
                                    return;
                                }
                                let date_str = asset_date_str.read().trim().to_string();
                                let date = match NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                                    Ok(d) => d,
                                    Err(_) => {
                                        asset_error.set(Some("取得年月日を正しく入力してください（例: 2025-04-01）".into()));
                                        return;
                                    }
                                };
                                let cost = match asset_cost_str.read().trim().parse::<i64>() {
                                    Ok(c) if c > 0 => c,
                                    _ => {
                                        asset_error.set(Some("取得価額を正の整数で入力してください".into()));
                                        return;
                                    }
                                };
                                let method = *asset_method.read();
                                if method == DepreciationMethod::LumpSum3Year && cost >= 200_000 {
                                    asset_error.set(Some("一括償却資産は取得価額20万円未満が対象です".into()));
                                    return;
                                }
                                if method == DepreciationMethod::ImmediateSmallAsset && cost >= 300_000 {
                                    asset_error.set(Some("少額減価償却資産特例は取得価額30万円未満が対象です".into()));
                                    return;
                                }

                                let asset = FixedAsset::new(
                                    name,
                                    date,
                                    cost,
                                    *asset_years.read(),
                                    method,
                                    *asset_ratio.read(),
                                    asset_account.read().clone(),
                                    asset_memo.read().trim().to_string(),
                                );

                                let mut doc = app.read().to_document();
                                doc.fixed_assets.push(asset);
                                if let Err(e) = save_year(&doc) {
                                    app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                } else {
                                    *app.write() = AppState::from_document(doc, Some("固定資産を登録しました".into()));
                                    asset_name.set(String::new());
                                    asset_cost_str.set(String::new());
                                    asset_memo.set(String::new());
                                }
                            },
                            "固定資産を追加"
                        }
                    }
                }

                // ---- 固定資産台帳 & 当期減価償却計算一覧 ----
                div { class: "card",
                    h2 { "{fiscal_year}年 固定資産台帳 & 減価償却費一覧" }
                    p { class: "muted",
                        "青色申告決算書 第3面「減価償却費の計算」に準拠した計算結果です。"
                    }

                    if dep_calcs.is_empty() {
                        div { class: "empty", "登録されている固定資産はありません。" }
                    } else {
                        div { class: "report-grid", style: "margin-bottom: 1.25rem;",
                            div { class: "stat",
                                div { class: "label", "資産取得総額" }
                                div { class: "value", "¥{format_yen(total_dep_cost)}" }
                            }
                            div { class: "stat",
                                div { class: "label", "期首帳簿価額 合計" }
                                div { class: "value", "¥{format_yen(total_dep_opening)}" }
                            }
                            div { class: "stat",
                                div { class: "label", "本年分普通償却額 合計" }
                                div { class: "value", "¥{format_yen(total_dep_ordinary)}" }
                            }
                            div { class: "stat",
                                div { class: "label", "本年分必要経費算入額（経費計上分）" }
                                div { class: "value", style: "color: var(--primary);", "¥{format_yen(total_dep_expense)}" }
                            }
                            div { class: "stat",
                                div { class: "label", "期末残高（未償却残高）合計" }
                                div { class: "value", "¥{format_yen(total_dep_closing)}" }
                            }
                        }

                        div { style: "overflow-x: auto;",
                            table { class: "ledger",
                                thead {
                                    tr {
                                        th { "資産名" }
                                        th { "取得年月" }
                                        th { "取得価額" }
                                        th { "償却方法" }
                                        th { "耐用年数" }
                                        th { "期首帳簿価額" }
                                        th { "本年償却額" }
                                        th { "事業割合" }
                                        th { "必要経費算入額" }
                                        th { "期末残高" }
                                        th { "操作" }
                                    }
                                }
                                tbody {
                                    for c in dep_calcs.iter() {
                                        tr {
                                            td {
                                                strong { "{c.name}" }
                                                span { class: "sub-tag", "{c.asset_account.label_ja()}" }
                                            }
                                            td { "{c.acquisition_date.format(\"%Y-%m\")}" }
                                            td { class: "amount", "¥{format_yen(c.acquisition_cost)}" }
                                            td { "{c.depreciation_method.label_ja()}" }
                                            td { "{c.useful_life_years}年" }
                                            td { class: "amount", "¥{format_yen(c.opening_book_value)}" }
                                            td { class: "amount", "¥{format_yen(c.ordinary_depreciation)}" }
                                            td { "{c.business_ratio_percent}%" }
                                            td { class: "amount", style: "color: var(--primary); font-weight: 700;", "¥{format_yen(c.business_expense)}" }
                                            td { class: "amount", "¥{format_yen(c.closing_book_value)}" }
                                            td {
                                                {
                                                    let asset_id = c.asset_id.clone();
                                                    let asset_name_val = c.name.clone();
                                                    rsx! {
                                                        button {
                                                            class: "btn btn-danger btn-sm",
                                                            onclick: move |_| {
                                                                let mut doc = app.read().to_document();
                                                                doc.fixed_assets.retain(|a| a.id != asset_id);
                                                                if let Err(e) = save_year(&doc) {
                                                                    app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                                                } else {
                                                                    *app.write() = AppState::from_document(
                                                                        doc,
                                                                        Some(format!("資産「{asset_name_val}」を削除しました")),
                                                                    );
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

                        div { class: "actions", style: "margin-top: 1.5rem; padding-top: 1rem; border-top: 1px solid var(--border);",
                            button {
                                class: "btn btn-primary",
                                onclick: move |_| {
                                    let mut doc = app.read().to_document();
                                    let count = apply_depreciation_entries(&mut doc);
                                    if let Err(e) = save_year(&doc) {
                                        app.write().error_message = Some(format!("仕訳の保存に失敗しました: {e}"));
                                    } else {
                                        *app.write() = AppState::from_document(
                                            doc,
                                            Some(format!("12月31日付の減価償却仕訳（{count}件）を仕訳帳へ自動反映しました")),
                                        );
                                    }
                                },
                                "⚡ 減価償却仕訳を一括生成・仕訳帳へ反映（12月31日付）"
                            }
                            button {
                                class: "btn btn-secondary btn-sm",
                                onclick: move |_| {
                                    let mut doc = app.read().to_document();
                                    let cleared = clear_depreciation_entries(&mut doc);
                                    if let Err(e) = save_year(&doc) {
                                        app.write().error_message = Some(format!("仕訳のクリアに失敗しました: {e}"));
                                    } else {
                                        *app.write() = AppState::from_document(
                                            doc,
                                            Some(format!("自動生成された減価償却仕訳（{cleared}件）を削除しました")),
                                        );
                                    }
                                },
                                "減価償却仕訳をクリア"
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
            },

            ClosingTab::Storage => rsx! {
                // ---- データ保存 & 相互運用 ----
                div { class: "card",
                    h2 { "単一ファイル SQLite (GnuCash互換) データベース状態" }
                    p { class: "muted",
                        "全年度の仕訳・マスタを単一のSQLiteデータベースで一元管理しています。GnuCashデスクトップで直接閲覧・相互運用可能な標準テーブルスキーマ（books, accounts, transactions, splits, slots）に準拠しています。"
                    }

                    if let Some(stats) = &sqlite_stats {
                        div { class: "report-grid", style: "margin-bottom: 1.25rem;",
                            div { class: "stat",
                                div { class: "label", "DBファイルサイズ" }
                                div { class: "value", "{format_file_size(stats.file_size_bytes)}" }
                            }
                            div { class: "stat",
                                div { class: "label", "登録年度数" }
                                div { class: "value", "{stats.registered_years.len()} 年度" }
                            }
                            div { class: "stat",
                                div { class: "label", "総取引件数（Transactions）" }
                                div { class: "value", "{stats.total_transactions} 件" }
                            }
                            div { class: "stat",
                                div { class: "label", "総明細行数（Splits）" }
                                div { class: "value", "{stats.total_splits} 行" }
                            }
                        }

                        div { style: "font-size: 0.88rem; color: var(--muted); margin-bottom: 1rem; word-break: break-all;",
                            strong { "保存先パス: " }
                            span { "{stats.db_path.display()}" }
                            br {}
                            strong { "登録済み年度: " }
                            span {
                                "{stats.registered_years.iter().map(|y| y.to_string()).collect::<Vec<_>>().join(\", \")}"
                            }
                        }
                    }

                    div { class: "actions", style: "padding-top: 0.75rem; border-top: 1px solid var(--border);",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                match exports_dir() {
                                    Ok(dir) => {
                                        let target = dir.join("accounting.gnucash");
                                        let sp = sqlite_path.clone();
                                        match open_sqlite_connection(&sp) {
                                            Ok(conn) => match export_gnucash_database(&conn, &target) {
                                                Ok(_) => {
                                                    app.write().status_message = Some(format!(
                                                        "GnuCash互換ファイルをエクスポートしました: {}",
                                                        target.display()
                                                    ));
                                                }
                                                Err(e) => {
                                                    app.write().error_message = Some(format!("エクスポートに失敗しました: {e}"));
                                                }
                                            },
                                            Err(e) => {
                                                app.write().error_message = Some(format!("DB接続に失敗しました: {e}"));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        app.write().error_message = Some(format!("出力先解決に失敗しました: {e}"));
                                    }
                                }
                            },
                            "📁 GnuCash 互換ファイル（.gnucash）をエクスポート"
                        }
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                match auto_migrate_if_needed() {
                                    Ok(Some(rep)) => {
                                        let msg = format!(
                                            "旧JSONから一括移行完了: {}年度（総仕訳{}件）",
                                            rep.migrated_years.len(),
                                            rep.total_entries
                                        );
                                        app.write().status_message = Some(msg);
                                    }
                                    Ok(None) => {
                                        app.write().status_message = Some("移行対象の旧JSONファイルはありません。既に最新です。".into());
                                    }
                                    Err(e) => {
                                        app.write().error_message = Some(format!("移行中にエラーが発生しました: {e}"));
                                    }
                                }
                            },
                            "🔄 旧年度別JSONデータから一括再移行"
                        }
                    }
                }

                // ---- Beancount (Plain Text Accounting) 連携 ----
                div { class: "card",
                    h2 { "Beancount プレーンテキスト会計 相互運用" }
                    p { class: "muted",
                        "オープンソースのプレーンテキスト会計規格「Beancount」形式でのエクスポートおよびインポートに対応しています。Gitでの行単位差分追跡や、Web UIツール「Fava」との連携が可能です。"
                    }

                    // Beancount エクスポート
                    h3 { class: "subheading", "{fiscal_year}年 Beancountエクスポート" }
                    div { class: "actions", style: "margin-bottom: 0.75rem;",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let doc = app.read().to_document();
                                let text = export_to_beancount(&doc);
                                beancount_preview.set(text.clone());

                                if let Ok(dir) = exports_dir() {
                                    let path = dir.join(format!("{fiscal_year}_ledger.beancount"));
                                    if let Ok(_) = std::fs::write(&path, text) {
                                        app.write().status_message = Some(format!("Beancountファイルを保存しました: {}", path.display()));
                                    }
                                }
                            },
                            "📋 Beancountテキストを生成 & ファイル保存"
                        }
                    }
                    if !beancount_preview.read().is_empty() {
                        div { class: "field",
                            label { "生成された Beancount プレビュー" }
                            textarea {
                                class: "csv-textarea",
                                style: "min-height: 140px;",
                                readonly: true,
                                value: "{beancount_preview.read()}",
                            }
                        }
                    }

                    // Beancount インポート
                    h3 { class: "subheading", style: "margin-top: 1.5rem;", "Beancountテキストのインポート" }
                    p { class: "muted", style: "font-size: 0.85rem;",
                        "Beancount形式のテキストを貼り付けると、{fiscal_year}年の仕訳として解析し、仕訳帳へ追加取り込みします。"
                    }
                    div { class: "field",
                        textarea {
                            class: "csv-textarea",
                            placeholder: "2025-04-01 * \"取引先\" \"売上\"\n  Assets:Bank:SMBC  100000 JPY\n  Income:Sales     -100000 JPY",
                            value: "{beancount_input}",
                            oninput: move |evt| beancount_input.set(evt.value()),
                        }
                    }
                    div { class: "actions", style: "margin-top: 0.75rem;",
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                let input_text = beancount_input.read().clone();
                                if input_text.trim().is_empty() {
                                    app.write().error_message = Some("Beancountテキストを入力してください".into());
                                    return;
                                }
                                match import_from_beancount(&input_text, fiscal_year) {
                                    Ok(entries) => {
                                        if entries.is_empty() {
                                            app.write().error_message = Some(format!("{fiscal_year}年に該当する有効な仕訳が見つかりませんでした"));
                                            return;
                                        }
                                        let count = entries.len();
                                        let mut doc = app.read().to_document();
                                        doc.entries.extend(entries);
                                        match save_year(&doc) {
                                            Ok(_) => {
                                                *app.write() = AppState::from_document(
                                                    doc,
                                                    Some(format!("Beancountから{count}件の仕訳を取り込みました")),
                                                );
                                                beancount_input.set(String::new());
                                            }
                                            Err(e) => {
                                                app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        app.write().error_message = Some(format!("パースに失敗しました: {e}"));
                                    }
                                }
                            },
                            "📥 Beancountテキストを取り込み（仕訳帳へ追加）"
                        }
                    }
                }
            },
            ClosingTab::AutoRules => rsx! {
                // ---- 自動仕訳ルール設定 ----
                div { class: "card",
                    h2 { "自動仕訳ルールの新規作成" }
                    p { class: "muted",
                        "銀行明細やクレジットカード明細の摘要に含まれるキーワードから、勘定科目を自動推論して仕訳を生成します。"
                    }

                    if let Some(err) = rule_error.read().clone() {
                        div { class: "error-banner", "{err}" }
                    }

                    div { class: "form-grid",
                        div { class: "field",
                            label { "ルール名" }
                            input {
                                r#type: "text",
                                placeholder: "例: Amazon 消耗品費, Suica 交通費",
                                value: "{new_rule_name}",
                                oninput: move |evt| new_rule_name.set(evt.value()),
                            }
                        }
                        div { class: "field",
                            label { "判定キーワード（マッチ対象）" }
                            input {
                                r#type: "text",
                                placeholder: "例: Amazon, Suica, Google, 電気",
                                value: "{new_rule_pattern}",
                                oninput: move |evt| new_rule_pattern.set(evt.value()),
                            }
                        }
                        div { class: "field",
                            label { "一致条件" }
                            select {
                                value: "{new_rule_match_type.read().label_ja()}",
                                onchange: move |evt| {
                                    match evt.value().as_str() {
                                        "前方一致" => new_rule_match_type.set(RuleMatchType::StartsWith),
                                        "完全一致" => new_rule_match_type.set(RuleMatchType::Exact),
                                        _ => new_rule_match_type.set(RuleMatchType::Contains),
                                    }
                                },
                                option { value: "部分一致", "部分一致（キーワードを含む）" }
                                option { value: "前方一致", "前方一致（キーワードから始まる）" }
                                option { value: "完全一致", "完全一致" }
                            }
                        }
                        div { class: "field",
                            label { "割り当てる勘定科目" }
                            select {
                                onchange: move |evt| {
                                    let state = app.read();
                                    if let Some(acc) = state.accounts.iter().find(|a| a.name == evt.value()) {
                                        new_rule_account.set(acc.code.clone());
                                    }
                                },
                                for acc in accounts_list.iter() {
                                    option {
                                        value: "{acc.name}",
                                        selected: *new_rule_account.read() == acc.code,
                                        "{acc.name} ({acc.nature.label_ja()})"
                                    }
                                }
                            }
                        }
                        div { class: "field",
                            label { "補助科目（任意）" }
                            input {
                                r#type: "text",
                                placeholder: "例: 書籍, クラウドサービス (省略可)",
                                value: "{new_rule_sub}",
                                oninput: move |evt| new_rule_sub.set(evt.value()),
                            }
                        }
                        div { class: "field",
                            label { "優先度（数値が大きいほど優先）" }
                            input {
                                r#type: "number",
                                value: "{new_rule_priority}",
                                oninput: move |evt| {
                                    if let Ok(v) = evt.value().parse::<i32>() {
                                        new_rule_priority.set(v);
                                    }
                                },
                            }
                        }
                    }

                    div { class: "actions", style: "margin-top: 0.8rem; display: flex; gap: 0.5rem;",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                rule_error.set(None);
                                let name = new_rule_name.read().trim().to_string();
                                let pattern = new_rule_pattern.read().trim().to_string();
                                if name.is_empty() {
                                    rule_error.set(Some("ルール名を入力してください".into()));
                                    return;
                                }
                                if pattern.is_empty() {
                                    rule_error.set(Some("判定キーワードを入力してください".into()));
                                    return;
                                }
                                let match_type = *new_rule_match_type.read();
                                let target_account = new_rule_account.read().clone();
                                let sub_raw = new_rule_sub.read().trim().to_string();
                                let target_sub = if sub_raw.is_empty() { None } else { Some(sub_raw) };
                                let priority = *new_rule_priority.read();

                                let mut rule = AutoBookingRule::new(name.clone(), pattern, target_account, target_sub);
                                rule.match_type = match_type;
                                rule.priority = priority;

                                let mut doc = app.read().to_document();
                                doc.auto_rules.push(rule);

                                if let Err(e) = save_year(&doc) {
                                    app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                } else {
                                    *app.write() = AppState::from_document(
                                        doc,
                                        Some(format!("自動仕訳ルール「{name}」を登録しました")),
                                    );
                                    new_rule_name.set(String::new());
                                    new_rule_pattern.set(String::new());
                                    new_rule_sub.set(String::new());
                                    new_rule_priority.set(0);
                                }
                            },
                            "➕ 自動仕訳ルールを登録"
                        }
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                let mut doc = app.read().to_document();
                                doc.auto_rules = default_auto_rules();
                                if let Err(e) = save_year(&doc) {
                                    app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                } else {
                                    *app.write() = AppState::from_document(
                                        doc,
                                        Some("自動仕訳ルールを標準プリセット（30+件）にリセットしました".into()),
                                    );
                                }
                            },
                            "🔄 標準ルールセット（30+件）に初期化"
                        }
                    }
                }

                div { class: "card",
                    div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.8rem;",
                        h2 { style: "margin: 0;", "登録済み自動仕訳ルール一覧（{auto_rules_list.len()} 件）" }
                    }
                    div { class: "table-container",
                        table { class: "data-table",
                            thead {
                                tr {
                                    th { style: "width: 60px;", "有効" }
                                    th { style: "width: 70px;", "優先度" }
                                    th { "ルール名" }
                                    th { "判定キーワード" }
                                    th { "条件" }
                                    th { "割当勘定科目" }
                                    th { "補助科目" }
                                    th { style: "text-align: right; width: 80px;", "操作" }
                                }
                            }
                            tbody {
                                for rule in auto_rules_list.iter() {
                                    {
                                        let rule_id = rule.id.clone();
                                        let rule_id_for_delete = rule.id.clone();
                                        let rule_name = rule.name.clone();
                                        let is_active = rule.is_active;
                                        rsx! {
                                            tr {
                                                key: "{rule.id}",
                                                td {
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: is_active,
                                                        onchange: move |_| {
                                                            let mut doc = app.read().to_document();
                                                            if let Some(r) = doc.auto_rules.iter_mut().find(|r| r.id == rule_id) {
                                                                r.is_active = !r.is_active;
                                                            }
                                                            if let Err(e) = save_year(&doc) {
                                                                app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                                            } else {
                                                                *app.write() = AppState::from_document(doc, None);
                                                            }
                                                        }
                                                    }
                                                }
                                                td {
                                                    span { class: "badge badge-neutral", "{rule.priority}" }
                                                }
                                                td {
                                                    strong { style: if !is_active { "color: var(--muted); text-decoration: line-through;" } else { "" }, "{rule.name}" }
                                                }
                                                td {
                                                    code { style: "background: var(--bg-code, #f0f4f8); padding: 0.15rem 0.4rem; border-radius: 4px;", "{rule.pattern}" }
                                                }
                                                td {
                                                    span { class: "badge badge-info", "{rule.match_type.label_ja()}" }
                                                }
                                                td {
                                                    span { class: "badge badge-expense", "{rule.target_account.label_ja()}" }
                                                }
                                                td {
                                                    if let Some(sub) = &rule.target_sub_account {
                                                        span { class: "sub-badge", "{sub}" }
                                                    } else {
                                                        span { class: "muted", "-" }
                                                    }
                                                }
                                                td { style: "text-align: right;",
                                                    button {
                                                        class: "btn btn-danger btn-sm",
                                                        title: "ルールを削除",
                                                        onclick: move |_| {
                                                            let mut doc = app.read().to_document();
                                                            doc.auto_rules.retain(|r| r.id != rule_id_for_delete);
                                                            if let Err(e) = save_year(&doc) {
                                                                app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                                            } else {
                                                                *app.write() = AppState::from_document(
                                                                    doc,
                                                                    Some(format!("ルール「{rule_name}」を削除しました")),
                                                                );
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
        }
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
