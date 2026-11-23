use crate::AppState;
use tax_return_application::core::compound::{CompoundSplit, CompoundTransaction};
use tax_return_application::core::csv_import::{
    parse_legacy_csv, parse_legacy_csv_str, CsvImportReport,
};
use tax_return_application::core::journal::{
    format_yen, validate_entry, AccountCode, JournalEntry,
};
use tax_return_application::core::universal_csv::{
    parse_universal_csv, parse_universal_csv_str, CsvMappingConfig, UniversalCsvReport,
};
use tax_return_application::storage::{load_year, save_year};
use chrono::{Local, NaiveDate};
use dioxus::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitDraft {
    pub id: String,
    pub is_debit: bool,
    pub account: AccountCode,
    pub sub_account: String,
    pub amount_str: String,
    pub memo: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    DateDesc,
    DateAsc,
}

impl SortOrder {
    pub fn label(self) -> &'static str {
        match self {
            Self::DateDesc => "日付（新しい順）",
            Self::DateAsc => "日付（古い順）",
        }
    }
}

#[component]
pub fn LedgerPage() -> Element {
    let mut app = use_context::<Signal<AppState>>();

    let today = Local::now().date_naive();
    let mut date = use_signal(|| today.format("%Y-%m-%d").to_string());
    let mut debit = use_signal(|| AccountCode::Cash);
    let mut debit_sub = use_signal(|| String::new());
    let mut credit = use_signal(|| AccountCode::Sales);
    let mut credit_sub = use_signal(|| String::new());
    let mut amount = use_signal(|| String::new());
    let mut description = use_signal(|| String::new());
    let mut form_error = use_signal(|| Option::<String>::None);
    let mut editing_id = use_signal(|| Option::<String>::None);

    let mut filter_text = use_signal(|| String::new());
    let mut filter_account = use_signal(|| Option::<AccountCode>::None);
    let mut filter_start_date = use_signal(|| String::new());
    let mut filter_end_date = use_signal(|| String::new());
    let mut sort_order = use_signal(|| SortOrder::DateDesc);

    let mut show_csv_import = use_signal(|| false);
    let mut csv_input_text = use_signal(|| String::new());
    let mut csv_report = use_signal(|| Option::<CsvImportReport>::None);
    let mut csv_replace_mode = use_signal(|| false);

    // 汎用CSVインポート & 自動仕訳ステート
    let mut show_universal_csv = use_signal(|| false);
    let mut universal_preset_idx = use_signal(|| 0usize);
    let mut universal_base_account = use_signal(|| AccountCode::Bank);
    let mut universal_base_sub = use_signal(|| String::new());
    let mut universal_input_text = use_signal(|| String::new());
    let mut universal_report = use_signal(|| Option::<UniversalCsvReport>::None);

    // 複合仕訳（諸口仕訳）ステート
    let mut is_compound_mode = use_signal(|| false);
    let mut compound_date = use_signal(|| today.format("%Y-%m-%d").to_string());
    let mut compound_desc = use_signal(|| String::new());
    let mut compound_splits = use_signal(|| {
        vec![
            SplitDraft {
                id: uuid::Uuid::new_v4().to_string(),
                is_debit: true,
                account: AccountCode::Supplies,
                sub_account: String::new(),
                amount_str: String::new(),
                memo: String::new(),
            },
            SplitDraft {
                id: uuid::Uuid::new_v4().to_string(),
                is_debit: false,
                account: AccountCode::Bank,
                sub_account: String::new(),
                amount_str: String::new(),
                memo: String::new(),
            },
        ]
    });
    let mut compound_error = use_signal(|| Option::<String>::None);

    let fiscal_year = app.read().fiscal_year;
    let entries_snapshot = app.read().entries.clone();
    let status = app.read().status_message.clone();
    let global_error = app.read().error_message.clone();

    let start_d = NaiveDate::parse_from_str(filter_start_date.read().trim(), "%Y-%m-%d").ok();
    let end_d = NaiveDate::parse_from_str(filter_end_date.read().trim(), "%Y-%m-%d").ok();

    let filtered: Vec<JournalEntry> = {
        let q = filter_text.read().trim().to_string();
        let account = filter_account.read().clone();
        let order = *sort_order.read();
        let mut items = entries_snapshot;
        items.sort_by(|a, b| match order {
            SortOrder::DateDesc => b
                .date
                .cmp(&a.date)
                .then_with(|| b.created_at.cmp(&a.created_at)),
            SortOrder::DateAsc => a
                .date
                .cmp(&b.date)
                .then_with(|| a.created_at.cmp(&b.created_at)),
        });
        items
            .into_iter()
            .filter(|e| {
                let text_ok = q.is_empty()
                    || e.description.contains(&q)
                    || e.debit.label_ja().contains(&q)
                    || e.credit.label_ja().contains(&q)
                    || e.debit_sub.as_deref().map(|s| s.contains(&q)).unwrap_or(false)
                    || e.credit_sub.as_deref().map(|s| s.contains(&q)).unwrap_or(false);
                let account_ok = match &account {
                    None => true,
                    Some(code) => &e.debit == code || &e.credit == code,
                };
                let start_ok = match start_d {
                    None => true,
                    Some(d) => e.date >= d,
                };
                let end_ok = match end_d {
                    None => true,
                    Some(d) => e.date <= d,
                };
                text_ok && account_ok && start_ok && end_ok
            })
            .collect()
    };

    let filtered_total: i64 = filtered.iter().map(|e| e.amount).sum();
    let is_editing = editing_id.read().is_some();
    let has_filter = !filter_text.read().trim().is_empty()
        || filter_account.read().is_some()
        || !filter_start_date.read().trim().is_empty()
        || !filter_end_date.read().trim().is_empty();

    let current_debit_label = debit.read().label_ja().to_string();
    let current_credit_label = credit.read().label_ja().to_string();
    let selected_filter_account = filter_account
        .read()
        .as_ref()
        .map(|c| c.label_ja().to_string())
        .unwrap_or_default();

    // 複合仕訳の貸借集計
    let splits_snapshot = compound_splits.read().clone();
    let mut deb_sum: i64 = 0;
    let mut cred_sum: i64 = 0;
    for s in &splits_snapshot {
        let amt: i64 = s.amount_str.trim().replace(',', "").parse().unwrap_or(0);
        if s.is_debit {
            deb_sum += amt;
        } else {
            cred_sum += amt;
        }
    }
    let balance_diff = (deb_sum - cred_sum).abs();
    let is_balanced = deb_sum > 0 && deb_sum == cred_sum;

    rsx! {
        h1 { class: "page-title", "{fiscal_year}年 仕訳帳" }

        if let Some(msg) = status {
            div { class: "status-banner", "{msg}" }
        }
        if let Some(msg) = global_error {
            div { class: "error-banner", "{msg}" }
        }

        div { class: "card",
            div { style: "display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.8rem;",
                div {
                    h2 { style: "margin: 0 0 0.5rem;", "会計年度" }
                    div { class: "year-select",
                        label { r#for: "fiscal-year", "年度" }
                        select {
                            id: "fiscal-year",
                            value: "{fiscal_year}",
                            onchange: move |evt| {
                                if let Ok(year) = evt.value().parse::<i32>() {
                                    editing_id.set(None);
                                    csv_report.set(None);
                                    universal_report.set(None);
                                    switch_fiscal_year(app, year);
                                }
                            },
                            for y in (fiscal_year - 3)..=(fiscal_year + 1) {
                                option { value: "{y}", selected: y == fiscal_year, "{y}年" }
                            }
                        }
                    }
                }
                div { style: "display: flex; gap: 0.5rem; flex-wrap: wrap;",
                    button {
                        class: if *show_universal_csv.read() { "btn btn-primary" } else { "btn btn-info" },
                        onclick: move |_| {
                            let cur = *show_universal_csv.read();
                            show_universal_csv.set(!cur);
                            if !cur {
                                show_csv_import.set(false);
                            }
                        },
                        if *show_universal_csv.read() { "▲ 銀行・カード自動仕訳を閉じる" } else { "⚡ 銀行・カードCSV自動仕訳" }
                    }
                    button {
                        class: if *show_csv_import.read() { "btn btn-secondary" } else { "btn btn-secondary" },
                        onclick: move |_| {
                            let cur = *show_csv_import.read();
                            show_csv_import.set(!cur);
                            if !cur {
                                show_universal_csv.set(false);
                            }
                        },
                        if *show_csv_import.read() { "▲ 旧CSVを閉じる" } else { "📥 旧Excel CSVインポート" }
                    }
                    button {
                        class: if *is_compound_mode.read() { "btn btn-warning" } else { "btn btn-secondary" },
                        onclick: move |_| {
                            let cur = *is_compound_mode.read();
                            is_compound_mode.set(!cur);
                        },
                        if *is_compound_mode.read() { "📋 単一仕訳入力に戻る" } else { "📑 複合仕訳（諸口）入力" }
                    }
                }
            }

            if *show_csv_import.read() {
                div { class: "csv-import-box",
                    h3 { style: "margin-top: 0; margin-bottom: 0.5rem; font-size: 0.98rem;", "旧Excel仕訳帳 CSVインポート" }
                    p { style: "font-size: 0.85rem; color: var(--muted); margin-bottom: 0.85rem; line-height: 1.5;",
                        "2行ヘッダー（日付, №, 借方..., 貸方..., 摘要...）形式のCSVを一括取り込みします。"
                        br {}
                        "Shift_JIS (Excel標準) および UTF-8 の両形式に自動対応しています。"
                    }

                    div { class: "actions", style: "margin-bottom: 0.75rem;",
                        button {
                            class: "btn btn-primary btn-sm",
                            onclick: move |_| {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("CSVファイル (*.csv, *.txt)", &["csv", "txt"])
                                    .pick_file()
                                {
                                    match std::fs::read(&path) {
                                        Ok(bytes) => {
                                            let report = parse_legacy_csv(&bytes, fiscal_year);
                                            csv_report.set(Some(report));
                                        }
                                        Err(e) => {
                                            app.write().error_message = Some(format!("ファイル読み込みに失敗しました: {e}"));
                                        }
                                    }
                                }
                            },
                            "📁 CSVファイルを選択して読み込み..."
                        }
                    }

                    div { style: "margin-top: 0.5rem;",
                        label { style: "font-size: 0.82rem; color: var(--muted); font-weight: 600; display: block; margin-bottom: 0.35rem;",
                            "またはCSVテキストを直接貼り付け:"
                        }
                        textarea {
                            class: "csv-textarea",
                            placeholder: "例:\n日付,№,借　　　　方,,,貸　　　　方,,,,,集計科目コード１,集計科目コード２,集計除外摘要文１,集計除外摘要文２,摘　　要,摘要２,摘要３\n日付,№,科目ｺｰﾄﾞ,勘定科目,金　　額,科目ｺｰﾄﾞ,勘定科目,金　　額,月,重複チェック,1,3,家事消費等,雑収入,摘要,摘要２,摘要３\n1月5日,1,104,その他の預金,\"165,000.00\",1,売上（収入）,\"165,000.00\",1,FALSE,165000,0,0,0,賃料:(2026年1月分),,",
                            value: "{csv_input_text}",
                            oninput: move |evt| csv_input_text.set(evt.value()),
                        }
                        div { class: "actions", style: "margin-top: 0.4rem;",
                            button {
                                class: "btn btn-secondary btn-sm",
                                disabled: csv_input_text.read().trim().is_empty(),
                                onclick: move |_| {
                                    let text = csv_input_text.read().clone();
                                    if !text.trim().is_empty() {
                                        let report = parse_legacy_csv_str(&text, fiscal_year);
                                        csv_report.set(Some(report));
                                    }
                                },
                                "貼り付けたテキストを解析"
                            }
                            if !csv_input_text.read().is_empty() {
                                button {
                                    class: "btn btn-secondary btn-sm",
                                    onclick: move |_| csv_input_text.set(String::new()),
                                    "クリア"
                                }
                            }
                        }
                    }

                    if let Some(report) = csv_report.read().clone() {
                        div { style: "margin-top: 1rem; border-top: 1px solid var(--border); padding-top: 0.8rem;",
                            h4 { style: "margin: 0 0 0.5rem; font-size: 0.92rem;", "解析結果プレビュー" }

                            div { class: "meta-row", style: "margin-bottom: 0.5rem;",
                                span {
                                    "取り込み可能: "
                                    strong { "{report.entries.len()} 件" }
                                    "（合計金額: ¥{format_yen(report.total_amount())}） / 対象行数: {report.total_data_rows} 行 / スキップ: {report.skipped_rows} 行"
                                }
                            }

                            if !report.warnings.is_empty() {
                                div { class: "warn-banner",
                                    strong { "⚠️ 科目マッピングに関する注意（{report.warnings.len()}件）:" }
                                    ul { style: "margin: 0.35rem 0 0; padding-left: 1.2rem; font-size: 0.85rem;",
                                        for w in report.warnings.iter().take(5) {
                                            li { "{w}" }
                                        }
                                        if report.warnings.len() > 5 {
                                            li { "...他 {report.warnings.len() - 5} 件" }
                                        }
                                    }
                                }
                            }

                            if !report.errors.is_empty() {
                                div { class: "error-banner",
                                    strong { "❌ 読み込み失敗行（{report.errors.len()}件）:" }
                                    ul { style: "margin: 0.35rem 0 0; padding-left: 1.2rem; font-size: 0.85rem;",
                                        for err in report.errors.iter().take(5) {
                                            li { "{err.line_number}行目: {err.message}" }
                                        }
                                        if report.errors.len() > 5 {
                                            li { "...他 {report.errors.len() - 5} 件" }
                                        }
                                    }
                                }
                            }

                            if !report.entries.is_empty() {
                                div { style: "margin-bottom: 0.75rem; overflow-x: auto;",
                                    table { class: "csv-preview-table",
                                        thead {
                                            tr {
                                                th { "日付" }
                                                th { "借方" }
                                                th { "貸方" }
                                                th { "金額" }
                                                th { "摘要" }
                                            }
                                        }
                                        tbody {
                                            for e in report.entries.iter().take(5) {
                                                tr {
                                                    td { "{e.date}" }
                                                    td { "{e.debit.label_ja()}" }
                                                    td { "{e.credit.label_ja()}" }
                                                    td { "¥{format_yen(e.amount)}" }
                                                    td { "{e.description}" }
                                                }
                                            }
                                        }
                                    }
                                    if report.entries.len() > 5 {
                                        div { style: "font-size: 0.8rem; color: var(--muted); margin-top: 0.3rem;",
                                            "...他 {report.entries.len() - 5} 件の仕訳"
                                        }
                                    }
                                }

                                div { style: "margin-bottom: 0.85rem; font-size: 0.88rem; display: flex; align-items: center; gap: 0.5rem;",
                                    input {
                                        r#type: "checkbox",
                                        id: "replace-mode-check",
                                        checked: *csv_replace_mode.read(),
                                        onchange: move |evt| csv_replace_mode.set(evt.value() == "true"),
                                    }
                                    label { r#for: "replace-mode-check",
                                        "既存の仕訳をすべて削除して置き換える（チェックを外した場合は既存仕訳の末尾に追加）"
                                    }
                                }

                                div { class: "actions",
                                    button {
                                        class: "btn btn-primary",
                                        onclick: move |_| {
                                            let count = report.entries.len();
                                            let replace = *csv_replace_mode.read();
                                            {
                                                let mut state = app.write();
                                                if replace {
                                                    state.entries = report.entries.clone();
                                                } else {
                                                    state.entries.extend(report.entries.clone());
                                                }
                                                state.error_message = None;
                                                if let Err(e) = persist(&state) {
                                                    state.error_message = Some(format!("インポート後の保存に失敗しました: {e}"));
                                                } else {
                                                    state.status_message = Some(format!("CSVから {} 件の仕訳を取り込み、保存しました", count));
                                                }
                                            }
                                            csv_report.set(None);
                                            csv_input_text.set(String::new());
                                            show_csv_import.set(false);
                                        },
                                        "✅ {report.entries.len()} 件の仕訳を取り込む"
                                    }
                                    button {
                                        class: "btn btn-secondary",
                                        onclick: move |_| {
                                            csv_report.set(None);
                                        },
                                        "キャンセル"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if *show_universal_csv.read() {
                div { class: "csv-import-box", style: "border: 2px solid #3b82f6; background: var(--card-bg, #ffffff); border-radius: 8px; padding: 1.2rem; margin-top: 1rem;",
                    h3 { style: "margin-top: 0; margin-bottom: 0.5rem; font-size: 1.05rem; color: #2563eb;",
                        "⚡ 汎用金融CSVインポート & ルールベース自動仕訳"
                    }
                    p { style: "font-size: 0.85rem; color: var(--muted); margin-bottom: 1rem; line-height: 1.5;",
                        "銀行（三菱UFJ・三井住友・みずほ・ゆうちょ・楽天銀行等）やクレジットカードの明細CSVを取り込み、自動仕訳ルールに基づいて摘要から借方・貸方・補助科目を自動推論します。"
                    }

                    div { class: "form-grid", style: "margin-bottom: 1rem;",
                        div { class: "field",
                            label { "インポート形式（プリセット）" }
                            select {
                                value: "{universal_preset_idx}",
                                onchange: move |evt| {
                                    if let Ok(idx) = evt.value().parse::<usize>() {
                                        universal_preset_idx.set(idx);
                                        if idx == 2 {
                                            universal_base_account.set(AccountCode::AccountsPayable);
                                        } else {
                                            universal_base_account.set(AccountCode::Bank);
                                        }
                                    }
                                },
                                option { value: "0", "🏦 銀行明細（出金・入金 2列形式）" }
                                option { value: "1", "🏦 銀行明細（取引金額 ±1列形式）" }
                                option { value: "2", "💳 クレジットカード明細（利用額形式）" }
                            }
                        }
                        div { class: "field",
                            label { "基礎勘定科目（決済口座 / 負債科目）" }
                            select {
                                onchange: move |evt| {
                                    let state = app.read();
                                    if let Some(acc) = state.accounts.iter().find(|a| a.name == evt.value()) {
                                        universal_base_account.set(acc.code.clone());
                                    }
                                },
                                for acc in app.read().accounts.iter() {
                                    option {
                                        value: "{acc.name}",
                                        selected: *universal_base_account.read() == acc.code,
                                        "{acc.name} ({acc.nature.label_ja()})"
                                    }
                                }
                            }
                        }
                        div { class: "field",
                            label { "基礎口座の補助科目（任意）" }
                            input {
                                r#type: "text",
                                placeholder: "例: 楽天銀行, 三菱UFJ, 楽天カード",
                                value: "{universal_base_sub}",
                                oninput: move |evt| universal_base_sub.set(evt.value()),
                            }
                        }
                    }

                    div { class: "actions", style: "margin-bottom: 0.8rem; display: flex; gap: 0.5rem;",
                        button {
                            class: "btn btn-primary btn-sm",
                            onclick: move |_| {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("CSVファイル (*.csv, *.txt)", &["csv", "txt"])
                                    .pick_file()
                                {
                                    match std::fs::read(&path) {
                                        Ok(bytes) => {
                                            let mut cfg = match *universal_preset_idx.read() {
                                                1 => CsvMappingConfig::preset_bank_single(),
                                                2 => CsvMappingConfig::preset_credit_card(),
                                                _ => CsvMappingConfig::preset_bank_dual(),
                                            };
                                            cfg.base_account = universal_base_account.read().clone();
                                            let sub = universal_base_sub.read().trim().to_string();
                                            cfg.base_sub_account = if sub.is_empty() { None } else { Some(sub) };

                                            let rules = app.read().auto_rules.clone();
                                            let report = parse_universal_csv(&bytes, &cfg, fiscal_year, &rules);
                                            universal_report.set(Some(report));
                                        }
                                        Err(e) => {
                                            app.write().error_message = Some(format!("ファイル読み込みに失敗しました: {e}"));
                                        }
                                    }
                                }
                            },
                            "📁 金融機関CSVファイルを選択して自動解析..."
                        }
                    }

                    div { style: "margin-top: 0.5rem;",
                        label { style: "font-size: 0.82rem; color: var(--muted); font-weight: 600; display: block; margin-bottom: 0.35rem;",
                            "またはCSVテキストを直接貼り付け:"
                        }
                        textarea {
                            class: "csv-textarea",
                            placeholder: "例 (銀行2列形式):\n年月日,お引出し,お預入れ,お取扱内容\n2026/01/10,5000,,Ａｍａｚｏｎ\n2026/01/15,,100000,売上振込\n2026/01/20,1200,,Ｓｕｉｃａチャージ",
                            value: "{universal_input_text}",
                            oninput: move |evt| universal_input_text.set(evt.value()),
                        }
                        div { class: "actions", style: "margin-top: 0.4rem; display: flex; gap: 0.5rem;",
                            button {
                                class: "btn btn-secondary btn-sm",
                                disabled: universal_input_text.read().trim().is_empty(),
                                onclick: move |_| {
                                    let text = universal_input_text.read().clone();
                                    if !text.trim().is_empty() {
                                        let mut cfg = match *universal_preset_idx.read() {
                                            1 => CsvMappingConfig::preset_bank_single(),
                                            2 => CsvMappingConfig::preset_credit_card(),
                                            _ => CsvMappingConfig::preset_bank_dual(),
                                        };
                                        cfg.base_account = universal_base_account.read().clone();
                                        let sub = universal_base_sub.read().trim().to_string();
                                        cfg.base_sub_account = if sub.is_empty() { None } else { Some(sub) };

                                        let rules = app.read().auto_rules.clone();
                                        let report = parse_universal_csv_str(&text, &cfg, fiscal_year, &rules);
                                        universal_report.set(Some(report));
                                    }
                                },
                                "貼り付けたテキストを自動解析"
                            }
                            if !universal_input_text.read().is_empty() {
                                button {
                                    class: "btn btn-secondary btn-sm",
                                    onclick: move |_| universal_input_text.set(String::new()),
                                    "クリア"
                                }
                            }
                        }
                    }

                    if let Some(rep) = universal_report.read().clone() {
                        div { style: "margin-top: 1rem; border-top: 1px solid var(--border); padding-top: 0.8rem;",
                            h4 { style: "margin: 0 0 0.5rem; font-size: 0.95rem; display: flex; align-items: center; gap: 0.5rem;",
                                "解析結果プレビュー"
                                span { class: "badge badge-success",
                                    "⚡ 自動仕訳合致: {rep.matched_rules_count} / {rep.rows.len()} 件"
                                }
                            }

                            div { class: "meta-row", style: "margin-bottom: 0.75rem; font-size: 0.88rem; display: flex; gap: 1rem; flex-wrap: wrap;",
                                span { "取込可能仕訳: " strong { "{rep.entries.len()} 件" } }
                                span { "入金合計: " strong { style: "color: #10b981;", "¥{format_yen(rep.total_income)}" } }
                                span { "出金合計: " strong { style: "color: #ef4444;", "¥{format_yen(rep.total_expense)}" } }
                            }

                            if !rep.errors.is_empty() {
                                div { class: "warn-banner", style: "margin-bottom: 0.8rem;",
                                    strong { "⚠️ 解析エラー行（{rep.errors.len()}件）:" }
                                    ul { style: "margin: 0.35rem 0 0; padding-left: 1.2rem; font-size: 0.85rem;",
                                        for err in rep.errors.iter().take(5) {
                                            li { "行 {err.line_number}: {err.message}" }
                                        }
                                    }
                                }
                            }

                            div { class: "table-container", style: "max-height: 280px; overflow-y: auto; margin-bottom: 1rem;",
                                table { class: "data-table",
                                    thead {
                                        tr {
                                            th { "日付" }
                                            th { "摘要・明細内容" }
                                            th { style: "text-align: right;", "金額" }
                                            th { "自動仕訳判定" }
                                            th { "借方科目 (＋補助)" }
                                            th { "貸方科目 (＋補助)" }
                                        }
                                    }
                                    tbody {
                                        for row in rep.rows.iter().take(100) {
                                            tr {
                                                td { "{row.date}" }
                                                td { "{row.description}" }
                                                td {
                                                    style: if row.is_expense { "text-align: right; color: #ef4444;" } else { "text-align: right; color: #10b981;" },
                                                    if row.is_expense { "▲ ¥{format_yen(row.amount)}" } else { "+ ¥{format_yen(row.amount)}" }
                                                }
                                                td {
                                                    if let Some(r_name) = &row.matched_rule_name {
                                                        span { class: "badge badge-success", "⚡ {r_name}" }
                                                    } else {
                                                        span { class: "badge badge-neutral", "基本科目" }
                                                    }
                                                }
                                                td {
                                                    span { class: "badge badge-expense", "{row.debit.label_ja()}" }
                                                    if let Some(sub) = &row.debit_sub {
                                                        span { class: "sub-badge", style: "margin-left: 0.3rem;", "{sub}" }
                                                    }
                                                }
                                                td {
                                                    span { class: "badge badge-income", "{row.credit.label_ja()}" }
                                                    if let Some(sub) = &row.credit_sub {
                                                        span { class: "sub-badge", style: "margin-left: 0.3rem;", "{sub}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            div { class: "actions", style: "display: flex; gap: 0.5rem;",
                                button {
                                    class: "btn btn-primary",
                                    disabled: rep.entries.is_empty(),
                                    onclick: move |_| {
                                        let mut doc = app.read().to_document();
                                        let count = rep.entries.len();
                                        doc.entries.extend(rep.entries.clone());
                                        if let Err(e) = save_year(&doc) {
                                            app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                        } else {
                                            *app.write() = AppState::from_document(
                                                doc,
                                                Some(format!("金融CSVから {count} 件の仕訳を自動取り込み・登録しました")),
                                            );
                                            universal_report.set(None);
                                            universal_input_text.set(String::new());
                                            show_universal_csv.set(false);
                                        }
                                    },
                                    "✅ {rep.entries.len()} 件の自動仕訳を一括取り込み"
                                }
                                button {
                                    class: "btn btn-secondary",
                                    onclick: move |_| universal_report.set(None),
                                    "キャンセル"
                                }
                            }
                        }
                    }
                }
            }
        }

        if *is_compound_mode.read() {
            // ---- 複合仕訳（諸口仕訳）入力フォーム ----
            div { class: "card", style: "border-left: 4px solid #3b82f6;",
                h2 { "複合仕訳（諸口仕訳）入力" }
                p { class: "muted", "1つの取引に対して複数の借方・貸方明細（スプリット）を登録します。借方合計と貸方合計が完全に一致したときに記帳できます。" }

                if let Some(err) = compound_error.read().clone() {
                    div { class: "error-banner", "{err}" }
                }

                div { class: "form-grid", style: "margin-bottom: 1rem;",
                    div { class: "field",
                        label { "取引日付" }
                        input {
                            r#type: "date",
                            value: "{compound_date}",
                            oninput: move |evt| compound_date.set(evt.value()),
                        }
                    }
                    div { class: "field", style: "grid-column: span 2;",
                        label { "取引全体の摘要（例: 役員報酬振込、売上入金及び手数料差引）" }
                        input {
                            r#type: "text",
                            placeholder: "取引内容を入力",
                            value: "{compound_desc}",
                            oninput: move |evt| compound_desc.set(evt.value()),
                        }
                    }
                }

                div { class: "table-container", style: "margin-bottom: 1rem;",
                    table { class: "data-table",
                        thead {
                            tr {
                                th { style: "width: 100px;", "借/貸" }
                                th { "勘定科目" }
                                th { "補助科目（任意）" }
                                th { style: "width: 140px; text-align: right;", "金額（円）" }
                                th { "明細メモ（任意）" }
                                th { style: "width: 60px; text-align: center;", "削除" }
                            }
                        }
                        tbody {
                            for (idx, split) in compound_splits.read().iter().enumerate() {
                                {
                                    let is_deb = split.is_debit;
                                    let cur_acc = split.account.clone();
                                    let cur_sub = split.sub_account.clone();
                                    let cur_amt = split.amount_str.clone();
                                    let cur_memo = split.memo.clone();
                                    rsx! {
                                        tr {
                                            key: "{split.id}",
                                            td {
                                                select {
                                                    value: if is_deb { "借方" } else { "貸方" },
                                                    onchange: move |evt| {
                                                        let mut splits = compound_splits.read().clone();
                                                        if let Some(item) = splits.get_mut(idx) {
                                                            item.is_debit = evt.value() == "借方";
                                                        }
                                                        compound_splits.set(splits);
                                                    },
                                                    option { value: "借方", "借方" }
                                                    option { value: "貸方", "貸方" }
                                                }
                                            }
                                            td {
                                                select {
                                                    onchange: move |evt| {
                                                        let state = app.read();
                                                        if let Some(acc) = state.accounts.iter().find(|a| a.name == evt.value()) {
                                                            let mut splits = compound_splits.read().clone();
                                                            if let Some(item) = splits.get_mut(idx) {
                                                                item.account = acc.code.clone();
                                                            }
                                                            compound_splits.set(splits);
                                                        }
                                                    },
                                                    for acc in app.read().accounts.iter() {
                                                        option {
                                                            value: "{acc.name}",
                                                            selected: cur_acc == acc.code,
                                                            "{acc.name}"
                                                        }
                                                    }
                                                }
                                            }
                                            td {
                                                input {
                                                    r#type: "text",
                                                    placeholder: "補助科目",
                                                    value: "{cur_sub}",
                                                    oninput: move |evt| {
                                                        let mut splits = compound_splits.read().clone();
                                                        if let Some(item) = splits.get_mut(idx) {
                                                            item.sub_account = evt.value();
                                                        }
                                                        compound_splits.set(splits);
                                                    }
                                                }
                                            }
                                            td {
                                                input {
                                                    r#type: "number",
                                                    style: "text-align: right;",
                                                    placeholder: "0",
                                                    value: "{cur_amt}",
                                                    oninput: move |evt| {
                                                        let mut splits = compound_splits.read().clone();
                                                        if let Some(item) = splits.get_mut(idx) {
                                                            item.amount_str = evt.value();
                                                        }
                                                        compound_splits.set(splits);
                                                    }
                                                }
                                            }
                                            td {
                                                input {
                                                    r#type: "text",
                                                    placeholder: "メモ",
                                                    value: "{cur_memo}",
                                                    oninput: move |evt| {
                                                        let mut splits = compound_splits.read().clone();
                                                        if let Some(item) = splits.get_mut(idx) {
                                                            item.memo = evt.value();
                                                        }
                                                        compound_splits.set(splits);
                                                    }
                                                }
                                            }
                                            td { style: "text-align: center;",
                                                button {
                                                    class: "btn btn-danger btn-sm",
                                                    disabled: compound_splits.read().len() <= 2,
                                                    onclick: move |_| {
                                                        let mut splits = compound_splits.read().clone();
                                                        if idx < splits.len() {
                                                            splits.remove(idx);
                                                        }
                                                        compound_splits.set(splits);
                                                    },
                                                    "×"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; flex-wrap: wrap; gap: 0.8rem;",
                    button {
                        class: "btn btn-secondary btn-sm",
                        onclick: move |_| {
                            let mut splits = compound_splits.read().clone();
                            splits.push(SplitDraft {
                                id: uuid::Uuid::new_v4().to_string(),
                                is_debit: true,
                                account: AccountCode::Supplies,
                                sub_account: String::new(),
                                amount_str: String::new(),
                                memo: String::new(),
                            });
                            compound_splits.set(splits);
                        },
                        "➕ 明細行（スプリット）を追加"
                    }

                    div { style: "display: flex; gap: 1.2rem; align-items: center; font-size: 0.92rem;",
                        span { "借方合計: " strong { "¥{format_yen(deb_sum)}" } }
                        span { "貸方合計: " strong { "¥{format_yen(cred_sum)}" } }
                        if is_balanced {
                            span { style: "color: #10b981; font-weight: bold;", "⚖️ 貸借一致 (¥{format_yen(deb_sum)})" }
                        } else {
                            span { style: "color: #ef4444; font-weight: bold;", "⚠️ 差額: ¥{format_yen(balance_diff)}" }
                        }
                    }
                }

                div { class: "actions",
                    button {
                        class: "btn btn-primary",
                        disabled: !is_balanced,
                        onclick: move |_| {
                            compound_error.set(None);
                            let desc = compound_desc.read().trim().to_string();
                            if desc.is_empty() {
                                compound_error.set(Some("摘要を入力してください".into()));
                                return;
                            }
                            let parsed_date = match NaiveDate::parse_from_str(compound_date.read().trim(), "%Y-%m-%d") {
                                Ok(d) => d,
                                Err(_) => {
                                    compound_error.set(Some("日付の形式が正しくありません".into()));
                                    return;
                                }
                            };
                            let splits_raw = compound_splits.read().clone();
                            let mut real_splits = Vec::new();
                            for s in splits_raw {
                                let amt = match s.amount_str.trim().replace(',', "").parse::<i64>() {
                                    Ok(v) if v > 0 => v,
                                    _ => {
                                        compound_error.set(Some("各明細の金額は1以上の正の整数を入力してください".into()));
                                        return;
                                    }
                                };
                                let sub = if s.sub_account.trim().is_empty() { None } else { Some(s.sub_account.trim().to_string()) };
                                let memo = if s.memo.trim().is_empty() { None } else { Some(s.memo.trim().to_string()) };
                                if s.is_debit {
                                    real_splits.push(CompoundSplit::debit(s.account, sub, amt, memo));
                                } else {
                                    real_splits.push(CompoundSplit::credit(s.account, sub, amt, memo));
                                }
                            }

                            match CompoundTransaction::new(parsed_date, desc.clone(), real_splits) {
                                Ok(ct) => {
                                    let new_entries = ct.to_journal_entries();
                                    let mut doc = app.read().to_document();
                                    doc.entries.extend(new_entries);
                                    if let Err(e) = save_year(&doc) {
                                        app.write().error_message = Some(format!("保存に失敗しました: {e}"));
                                    } else {
                                        *app.write() = AppState::from_document(
                                            doc,
                                            Some(format!("複合仕訳「{desc}」を登録しました")),
                                        );
                                        compound_desc.set(String::new());
                                    }
                                }
                                Err(e) => {
                                    compound_error.set(Some(e.to_string()));
                                }
                            }
                        },
                        "📑 複合仕訳を登録"
                    }
                }
            }
        } else {
        div { class: "card",
            h2 {
                if is_editing {
                    "仕訳の編集（ID: "
                    { editing_id.read().clone().unwrap_or_default() }
                    "）"
                } else {
                    "仕訳入力"
                }
            }
            if let Some(err) = form_error.read().clone() {
                div { class: "error-banner", "{err}" }
            }
            div { class: "form-grid",
                div { class: "field",
                    label { "日付" }
                    input {
                        r#type: "date",
                        value: "{date}",
                        oninput: move |evt| date.set(evt.value()),
                    }
                }
                div { class: "field",
                    label { "借方科目" }
                    select {
                        value: "{current_debit_label}",
                        onchange: move |evt| {
                            let state = app.read();
                            if let Some(acc) = state.accounts.iter().find(|a| a.name == evt.value()) {
                                debit.set(acc.code.clone());
                            }
                        },
                        for acc in app.read().accounts.iter() {
                            option {
                                value: "{acc.name}",
                                selected: debit.read().label_ja() == acc.name,
                                "{acc.name}"
                            }
                        }
                    }
                }
                div { class: "field",
                    label { "借方 補助科目" }
                    input {
                        r#type: "text",
                        placeholder: "任意（例: 三井住友銀行）",
                        list: "debit-sub-list",
                        value: "{debit_sub}",
                        oninput: move |evt| debit_sub.set(evt.value()),
                    }
                    datalist { id: "debit-sub-list",
                        {
                            let d_code = debit.read().clone();
                            let subs = app.read().accounts.iter().find(|a| a.code == d_code).map(|a| a.sub_accounts.clone()).unwrap_or_default();
                            rsx! {
                                for s in subs {
                                    option { value: "{s}" }
                                }
                            }
                        }
                    }
                }
                div { class: "field",
                    label { "貸方科目" }
                    select {
                        value: "{current_credit_label}",
                        onchange: move |evt| {
                            let state = app.read();
                            if let Some(acc) = state.accounts.iter().find(|a| a.name == evt.value()) {
                                credit.set(acc.code.clone());
                            }
                        },
                        for acc in app.read().accounts.iter() {
                            option {
                                value: "{acc.name}",
                                selected: credit.read().label_ja() == acc.name,
                                "{acc.name}"
                            }
                        }
                    }
                }
                div { class: "field",
                    label { "貸方 補助科目" }
                    input {
                        r#type: "text",
                        placeholder: "任意（例: 楽天カード）",
                        list: "credit-sub-list",
                        value: "{credit_sub}",
                        oninput: move |evt| credit_sub.set(evt.value()),
                    }
                    datalist { id: "credit-sub-list",
                        {
                            let c_code = credit.read().clone();
                            let subs = app.read().accounts.iter().find(|a| a.code == c_code).map(|a| a.sub_accounts.clone()).unwrap_or_default();
                            rsx! {
                                for s in subs {
                                    option { value: "{s}" }
                                }
                            }
                        }
                    }
                }
                div { class: "field",
                    label { "金額（円）" }
                    input {
                        r#type: "number",
                        min: "1",
                        step: "1",
                        placeholder: "例: 10000",
                        value: "{amount}",
                        oninput: move |evt| amount.set(evt.value()),
                    }
                }
                div { class: "field wide",
                    label { "摘要" }
                    input {
                        r#type: "text",
                        placeholder: "取引の内容",
                        value: "{description}",
                        oninput: move |evt| description.set(evt.value()),
                    }
                }
            }
            div { class: "actions", style: "margin-top: 0.9rem;",
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        form_error.set(None);
                        let parsed_date = match NaiveDate::parse_from_str(date.read().trim(), "%Y-%m-%d") {
                            Ok(d) => d,
                            Err(_) => {
                                form_error.set(Some("日付の形式が正しくありません".into()));
                                return;
                            }
                        };
                        let parsed_amount = match amount.read().trim().parse::<i64>() {
                            Ok(v) => v,
                            Err(_) => {
                                form_error.set(Some("金額は整数で入力してください".into()));
                                return;
                            }
                        };
                        let d_sub = debit_sub.read().trim().to_string();
                        let c_sub = credit_sub.read().trim().to_string();
                        let mut entry = JournalEntry::new_with_sub(
                            parsed_date,
                            debit.read().clone(),
                            if d_sub.is_empty() { None } else { Some(d_sub) },
                            credit.read().clone(),
                            if c_sub.is_empty() { None } else { Some(c_sub) },
                            parsed_amount,
                            description.read().clone(),
                        );
                        let year = app.read().fiscal_year;
                        if let Err(err) = validate_entry(&entry, year) {
                            form_error.set(Some(err.to_string()));
                            return;
                        }

                        {
                            let mut state = app.write();
                            if let Some(current_id) = editing_id.read().clone() {
                                entry.id = current_id.clone();
                                if let Some(target) = state.entries.iter_mut().find(|e| e.id == current_id) {
                                    *target = entry;
                                }
                                state.error_message = None;
                                if let Err(e) = persist(&state) {
                                    state.error_message = Some(format!("更新後の保存に失敗しました: {e}"));
                                } else {
                                    state.status_message = Some("仕訳を更新して保存しました".into());
                                }
                            } else {
                                state.entries.push(entry);
                                state.error_message = None;
                                if let Err(e) = persist(&state) {
                                    state.error_message = Some(format!("保存に失敗しました: {e}"));
                                } else {
                                    state.status_message = Some("仕訳を登録して保存しました".into());
                                }
                            }
                        }
                        editing_id.set(None);
                        debit_sub.set(String::new());
                        credit_sub.set(String::new());
                        amount.set(String::new());
                        description.set(String::new());
                    },
                    if is_editing { "更新" } else { "登録" }
                }
                if is_editing {
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| {
                            editing_id.set(None);
                            form_error.set(None);
                            debit_sub.set(String::new());
                            credit_sub.set(String::new());
                            amount.set(String::new());
                            description.set(String::new());
                        },
                        "キャンセル"
                    }
                }
            }
        }
        }

        div { class: "card",
            h2 { "仕訳一覧" }
            div { class: "filter-bar",
                input {
                    r#type: "text",
                    placeholder: "摘要・科目で検索",
                    value: "{filter_text}",
                    oninput: move |evt| filter_text.set(evt.value()),
                }
                select {
                    value: "{selected_filter_account}",
                    onchange: move |evt| {
                        let v = evt.value();
                        if v.is_empty() {
                            filter_account.set(None);
                        } else {
                            let state = app.read();
                            if let Some(acc) = state.accounts.iter().find(|a| a.name == v) {
                                filter_account.set(Some(acc.code.clone()));
                            }
                        }
                    },
                    option { value: "", "すべての科目" }
                    for acc in app.read().accounts.iter() {
                        option { value: "{acc.name}", "{acc.name}" }
                    }
                }
                div { style: "display: flex; align-items: center; gap: 0.35rem;",
                    label { style: "font-size: 0.8rem; color: var(--muted);", "期間:" }
                    input {
                        r#type: "date",
                        value: "{filter_start_date}",
                        oninput: move |evt| filter_start_date.set(evt.value()),
                    }
                    span { "〜" }
                    input {
                        r#type: "date",
                        value: "{filter_end_date}",
                        oninput: move |evt| filter_end_date.set(evt.value()),
                    }
                }
                select {
                    value: match *sort_order.read() {
                        SortOrder::DateDesc => "desc",
                        SortOrder::DateAsc => "asc",
                    },
                    onchange: move |evt| {
                        if evt.value() == "asc" {
                            sort_order.set(SortOrder::DateAsc);
                        } else {
                            sort_order.set(SortOrder::DateDesc);
                        }
                    },
                    option { value: "desc", "{SortOrder::DateDesc.label()}" }
                    option { value: "asc", "{SortOrder::DateAsc.label()}" }
                }
                if has_filter {
                    button {
                        class: "btn btn-secondary btn-sm",
                        onclick: move |_| {
                            filter_text.set(String::new());
                            filter_account.set(None);
                            filter_start_date.set(String::new());
                            filter_end_date.set(String::new());
                        },
                        "条件クリア"
                    }
                }
            }
            div { class: "meta-row",
                span {
                    "{filtered.len()} 件表示（合計金額: ¥{format_yen(filtered_total)}） / 全 {app.read().entries.len()} 件"
                }
            }

            if filtered.is_empty() {
                div { class: "empty", "仕訳がありません。上のフォームから登録してください。" }
            } else {
                table { class: "ledger",
                    thead {
                        tr {
                            th { "日付" }
                            th { "借方" }
                            th { "貸方" }
                            th { "金額" }
                            th { "摘要" }
                            th { style: "text-align: right;", "操作" }
                        }
                    }
                    tbody {
                        for entry in filtered.into_iter() {
                            tr {
                                key: "{entry.id}",
                                td { "{entry.date}" }
                                td {
                                    span { "{entry.debit.label_ja()}" }
                                    if let Some(sub) = &entry.debit_sub {
                                        span { class: "sub-tag", "({sub})" }
                                    }
                                }
                                td {
                                    span { "{entry.credit.label_ja()}" }
                                    if let Some(sub) = &entry.credit_sub {
                                        span { class: "sub-tag", "({sub})" }
                                    }
                                }
                                td { class: "amount", "{format_yen(entry.amount)}" }
                                td { "{entry.description}" }
                                td {
                                    div { class: "row-actions",
                                        {
                                            let e = entry.clone();
                                            rsx! {
                                                button {
                                                    class: "btn btn-secondary btn-sm",
                                                    onclick: move |_| {
                                                        editing_id.set(Some(e.id.clone()));
                                                        date.set(e.date.format("%Y-%m-%d").to_string());
                                                        debit.set(e.debit.clone());
                                                        debit_sub.set(e.debit_sub.clone().unwrap_or_default());
                                                        credit.set(e.credit.clone());
                                                        credit_sub.set(e.credit_sub.clone().unwrap_or_default());
                                                        amount.set(e.amount.to_string());
                                                        description.set(e.description.clone());
                                                        form_error.set(None);
                                                    },
                                                    "編集"
                                                }
                                            }
                                        }
                                        {
                                            let id = entry.id.clone();
                                            rsx! {
                                                button {
                                                    class: "btn btn-danger btn-sm",
                                                    onclick: move |_| {
                                                        if *editing_id.read() == Some(id.clone()) {
                                                            editing_id.set(None);
                                                            debit_sub.set(String::new());
                                                            credit_sub.set(String::new());
                                                            amount.set(String::new());
                                                            description.set(String::new());
                                                        }
                                                        let mut state = app.write();
                                                        state.entries.retain(|e| e.id != id);
                                                        state.error_message = None;
                                                        if let Err(e) = persist(&state) {
                                                            state.error_message = Some(format!("保存に失敗しました: {e}"));
                                                        } else {
                                                            state.status_message = Some("仕訳を削除して保存しました".into());
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
        }
    }
}


fn persist(state: &AppState) -> Result<(), String> {
    save_year(&state.to_document()).map_err(|e| e.to_string())
}

fn switch_fiscal_year(mut app: Signal<AppState>, year: i32) {
    match load_year(year) {
        Ok(doc) => {
            *app.write() = AppState::from_document(
                doc,
                Some(format!("{year}年のデータを読み込みました")),
            );
        }
        Err(e) => {
            app.write().error_message = Some(format!("読み込みに失敗しました: {e}"));
        }
    }
}
