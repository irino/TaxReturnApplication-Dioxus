use crate::AppState;
use tax_return_application::core::csv_import::{
    parse_legacy_csv, parse_legacy_csv_str, CsvImportReport,
};
use tax_return_application::core::journal::{
    format_yen, validate_entry, AccountCode, JournalEntry,
};
use tax_return_application::storage::{load_year, save_year};
use chrono::{Local, NaiveDate};
use dioxus::prelude::*;

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
    let mut credit = use_signal(|| AccountCode::Sales);
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

    let fiscal_year = app.read().fiscal_year;
    let entries_snapshot = app.read().entries.clone();
    let status = app.read().status_message.clone();
    let global_error = app.read().error_message.clone();

    let start_d = NaiveDate::parse_from_str(filter_start_date.read().trim(), "%Y-%m-%d").ok();
    let end_d = NaiveDate::parse_from_str(filter_end_date.read().trim(), "%Y-%m-%d").ok();

    let filtered: Vec<JournalEntry> = {
        let q = filter_text.read().trim().to_string();
        let account = *filter_account.read();
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
                    || e.credit.label_ja().contains(&q);
                let account_ok = match account {
                    None => true,
                    Some(code) => e.debit == code || e.credit == code,
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

    let current_debit_label = (*debit.read()).label_ja();
    let current_credit_label = (*credit.read()).label_ja();
    let selected_filter_account = (*filter_account.read())
        .map(|c| c.label_ja())
        .unwrap_or_default();

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
                                    switch_fiscal_year(app, year);
                                }
                            },
                            for y in (fiscal_year - 3)..=(fiscal_year + 1) {
                                option { value: "{y}", selected: y == fiscal_year, "{y}年" }
                            }
                        }
                    }
                }
                div {
                    button {
                        class: if *show_csv_import.read() { "btn btn-secondary" } else { "btn btn-primary" },
                        onclick: move |_| {
                            let cur = *show_csv_import.read();
                            show_csv_import.set(!cur);
                        },
                        if *show_csv_import.read() { "▲ CSVインポートを閉じる" } else { "📥 CSVインポート（2行ヘッダー対応）" }
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
        }

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
                    label { "借方" }
                    select {
                        value: "{current_debit_label}",
                        onchange: move |evt| {
                            if let Some(code) = account_from_label(&evt.value()) {
                                debit.set(code);
                            }
                        },
                        for code in AccountCode::all() {
                            option {
                                value: "{code.label_ja()}",
                                selected: *debit.read() == *code,
                                "{code.label_ja()}"
                            }
                        }
                    }
                }
                div { class: "field",
                    label { "貸方" }
                    select {
                        value: "{current_credit_label}",
                        onchange: move |evt| {
                            if let Some(code) = account_from_label(&evt.value()) {
                                credit.set(code);
                            }
                        },
                        for code in AccountCode::all() {
                            option {
                                value: "{code.label_ja()}",
                                selected: *credit.read() == *code,
                                "{code.label_ja()}"
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
                        let mut entry = JournalEntry::new(
                            parsed_date,
                            *debit.read(),
                            *credit.read(),
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
                            amount.set(String::new());
                            description.set(String::new());
                        },
                        "キャンセル"
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
                        } else if let Some(code) = account_from_label(&v) {
                            filter_account.set(Some(code));
                        }
                    },
                    option { value: "", "すべての科目" }
                    for code in AccountCode::all() {
                        option { value: "{code.label_ja()}", "{code.label_ja()}" }
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
                                td { "{entry.debit.label_ja()}" }
                                td { "{entry.credit.label_ja()}" }
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
                                                        debit.set(e.debit);
                                                        credit.set(e.credit);
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


fn account_from_label(label: &str) -> Option<AccountCode> {
    AccountCode::all()
        .iter()
        .copied()
        .find(|c| c.label_ja() == label)
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
