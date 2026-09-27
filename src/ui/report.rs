//! Tax document summary and PDF export.

use crate::AppState;
use dioxus::prelude::*;
use tax_return_application::core::journal::format_yen;
use tax_return_application::core::tax_calc::{
    build_tax_report, BlueSpecialDeductionKind,
};
use tax_return_application::pdf::{export_blue_return_pdf, export_form_b_pdf};
use tax_return_application::storage::{exports_dir, save_year};

#[component]
pub fn ReportPage() -> Element {
    let mut app = use_context::<Signal<AppState>>();

    let state = app.read().clone();
    let report = build_tax_report(
        &state.entries,
        state.fiscal_year,
        state.special_deduction,
    );
    let blue = report.blue;
    let form_b = report.form_b;
    let status = state.status_message.clone();
    let global_error = state.error_message.clone();
    let taxpayer = state.taxpayer.clone();
    let special = state.special_deduction;

    rsx! {
        h1 { class: "page-title", "{blue.fiscal_year}年 帳票・集計" }

        if let Some(msg) = status {
            div { class: "status-banner", "{msg}" }
        }
        if let Some(msg) = global_error {
            div { class: "error-banner", "{msg}" }
        }

        div { class: "card",
            h2 { "納税者情報" }
            p { class: "muted", "PDF出力時に印字されます。変更は自動保存されます。" }
            div { class: "form-grid",
                div { class: "field",
                    label { "氏名" }
                    input {
                        r#type: "text",
                        value: "{taxpayer.name}",
                        oninput: move |evt| {
                            app.write().taxpayer.name = evt.value();
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "屋号" }
                    input {
                        r#type: "text",
                        value: "{taxpayer.business_name}",
                        oninput: move |evt| {
                            app.write().taxpayer.business_name = evt.value();
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field wide",
                    label { "住所" }
                    input {
                        r#type: "text",
                        value: "{taxpayer.address}",
                        oninput: move |evt| {
                            app.write().taxpayer.address = evt.value();
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "電話番号" }
                    input {
                        r#type: "text",
                        value: "{taxpayer.phone}",
                        oninput: move |evt| {
                            app.write().taxpayer.phone = evt.value();
                            persist_quiet(app);
                        },
                    }
                }
            }
        }

        div { class: "card",
            h2 { "青色申告特別控除" }
            div { class: "field",
                label { "控除区分" }
                select {
                    onchange: move |evt| {
                        let v = evt.value();
                        let kind = match v.as_str() {
                            "none" => BlueSpecialDeductionKind::None,
                            "yen650k" => BlueSpecialDeductionKind::Yen650k,
                            _ => BlueSpecialDeductionKind::Yen100k,
                        };
                        {
                            let mut state = app.write();
                            state.special_deduction = kind;
                            state.status_message = Some(format!("特別控除を「{}」に設定しました", kind.label_ja()));
                        }
                        persist_quiet(app);
                    },
                    option {
                        value: "none",
                        selected: special == BlueSpecialDeductionKind::None,
                        "{BlueSpecialDeductionKind::None.label_ja()}"
                    }
                    option {
                        value: "yen100k",
                        selected: special == BlueSpecialDeductionKind::Yen100k,
                        "{BlueSpecialDeductionKind::Yen100k.label_ja()}"
                    }
                    option {
                        value: "yen650k",
                        selected: special == BlueSpecialDeductionKind::Yen650k,
                        "{BlueSpecialDeductionKind::Yen650k.label_ja()}"
                    }
                }
            }
        }

        div { class: "card",
            h2 { "青色申告決算書（損益計算）" }
            p { class: "muted",
                "仕訳から自動集計した概算です。公式様式そのものではありません。"
            }
            div { class: "report-grid",
                div { class: "stat",
                    div { class: "label", "売上高" }
                    div { class: "value", "¥{format_yen(blue.sales)}" }
                }
                div { class: "stat",
                    div { class: "label", "仕入高" }
                    div { class: "value", "¥{format_yen(blue.cost_of_sales)}" }
                }
                div { class: "stat",
                    div { class: "label", "売上総利益" }
                    div { class: "value", "¥{format_yen(blue.gross_profit)}" }
                }
                div { class: "stat",
                    div { class: "label", "経費合計" }
                    div { class: "value", "¥{format_yen(blue.total_expenses)}" }
                }
                div { class: "stat",
                    div { class: "label", "特別控除前所得" }
                    div { class: "value", "¥{format_yen(blue.ordinary_income_before_deduction)}" }
                }
                div { class: "stat",
                    div { class: "label", "青色特別控除" }
                    div { class: "value", "¥{format_yen(blue.special_deduction)}" }
                }
                div { class: "stat",
                    div { class: "label", "事業所得" }
                    div { class: "value", "¥{format_yen(blue.business_income)}" }
                }
            }

            h3 { class: "subheading", "経費内訳" }
            table { class: "ledger",
                thead {
                    tr {
                        th { "科目" }
                        th { "金額" }
                    }
                }
                tbody {
                    for (name, amount) in blue.expenses.lines() {
                        tr {
                            key: "{name}",
                            td { "{name}" }
                            td { class: "amount", "¥{format_yen(amount)}" }
                        }
                    }
                }
            }
        }

        div { class: "card",
            h2 { "確定申告書B（所得税概算）" }
            p { class: "muted",
                "事業所得と基礎控除のみを反映した概算です。社会保険料控除・扶養控除等は未対応です。"
            }
            div { class: "report-grid",
                div { class: "stat",
                    div { class: "label", "所得金額合計" }
                    div { class: "value", "¥{format_yen(form_b.total_income)}" }
                }
                div { class: "stat",
                    div { class: "label", "基礎控除" }
                    div { class: "value", "¥{format_yen(form_b.basic_deduction)}" }
                }
                div { class: "stat",
                    div { class: "label", "課税所得" }
                    div { class: "value", "¥{format_yen(form_b.taxable_income)}" }
                }
                div { class: "stat",
                    div { class: "label", "所得税" }
                    div { class: "value", "¥{format_yen(form_b.income_tax)}" }
                }
                div { class: "stat",
                    div { class: "label", "復興特別所得税" }
                    div { class: "value", "¥{format_yen(form_b.reconstruction_surtax)}" }
                }
                div { class: "stat",
                    div { class: "label", "所得税等合計" }
                    div { class: "value", "¥{format_yen(form_b.total_income_tax)}" }
                }
            }
        }

        div { class: "card",
            h2 { "PDF出力" }
            p { class: "muted",
                "アプリデータフォルダの exports に PDF を保存します。日本語フォント（IPAゴシック等）が必要です。"
            }
            div { class: "actions",
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        export_pdfs(app, ExportKind::Blue);
                    },
                    "青色申告決算書 PDF"
                }
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        export_pdfs(app, ExportKind::FormB);
                    },
                    "確定申告書B PDF"
                }
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        export_pdfs(app, ExportKind::Both);
                    },
                    "両方出力"
                }
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        open_export_directory();
                    },
                    "保存先フォルダを開く"
                }
            }
        }

        div { class: "card",
            h2 { "科目別残高" }
            if blue.by_account.is_empty() {
                div { class: "empty", "集計対象の仕訳がありません。" }
            } else {
                table { class: "ledger",
                    thead {
                        tr {
                            th { "科目" }
                            th { "金額" }
                        }
                    }
                    tbody {
                        for (name, amount) in blue.by_account.iter() {
                            tr {
                                key: "{name}",
                                td { "{name}" }
                                td { class: "amount", "¥{format_yen(*amount)}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ExportKind {
    Blue,
    FormB,
    Both,
}

fn persist_quiet(mut app: Signal<AppState>) {
    let doc = app.read().to_document();
    if let Err(e) = save_year(&doc) {
        app.write().error_message = Some(format!("保存に失敗しました: {e}"));
    }
}

fn export_pdfs(mut app: Signal<AppState>, kind: ExportKind) {
    let state = app.read().clone();
    let report = build_tax_report(
        &state.entries,
        state.fiscal_year,
        state.special_deduction,
    );

    let dir = match exports_dir() {
        Ok(d) => d,
        Err(e) => {
            app.write().error_message = Some(format!("出力先を解決できません: {e}"));
            return;
        }
    };

    let year = state.fiscal_year;
    let mut messages = Vec::new();
    let mut errors = Vec::new();

    let do_blue = matches!(kind, ExportKind::Blue | ExportKind::Both);
    let do_form = matches!(kind, ExportKind::FormB | ExportKind::Both);

    if do_blue {
        let path = dir.join(format!("{year}_青色申告決算書.pdf"));
        match export_blue_return_pdf(&report.blue, &state.taxpayer, &path) {
            Ok(p) => messages.push(format!("青色: {}", p.display())),
            Err(e) => errors.push(format!("青色PDF: {e}")),
        }
    }
    if do_form {
        let path = dir.join(format!("{year}_確定申告書B.pdf"));
        match export_form_b_pdf(&report.form_b, &state.taxpayer, &path) {
            Ok(p) => messages.push(format!("申告書B: {}", p.display())),
            Err(e) => errors.push(format!("申告書B PDF: {e}")),
        }
    }

    let mut state = app.write();
    if !errors.is_empty() {
        state.error_message = Some(errors.join(" / "));
        state.status_message = None;
    } else {
        state.error_message = None;
        state.status_message = Some(format!("PDFを出力しました: {}", messages.join(" , ")));
    }
}

fn open_export_directory() {
    if let Ok(dir) = exports_dir() {
        let _ = std::fs::create_dir_all(&dir);
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&dir).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    }
}

