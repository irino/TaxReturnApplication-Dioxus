//! Tax document summary and PDF export.

use crate::AppState;
use dioxus::prelude::*;
use tax_return_application::core::deductions::{
    SpouseDeductionType, WidowOrSingleParentType,
};
use tax_return_application::core::journal::format_yen;
use tax_return_application::core::tax_calc::{
    build_tax_report_with_deductions, BlueSpecialDeductionKind,
};
use tax_return_application::pdf::{export_blue_return_pdf, export_form_b_pdf};
use tax_return_application::storage::{exports_dir, save_year};

#[component]
pub fn ReportPage() -> Element {
    let mut app = use_context::<Signal<AppState>>();

    let state = app.read().clone();
    let report = build_tax_report_with_deductions(
        &state.entries,
        state.fiscal_year,
        state.special_deduction,
        &state.deductions,
    );
    let blue = report.blue;
    let form_b = report.form_b;
    let status = state.status_message.clone();
    let global_error = state.error_message.clone();
    let taxpayer = state.taxpayer.clone();
    let special = state.special_deduction;
    let deductions = state.deductions.clone();
    let d_detail = form_b.deductions_detail.clone();

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
                    if blue.itemized_expenses.is_empty() {
                        for (name, amount) in blue.expenses.lines() {
                            tr {
                                key: "{name}",
                                td { "{name}" }
                                td { class: "amount", "¥{format_yen(amount)}" }
                            }
                        }
                    } else {
                        for (name, amount) in blue.itemized_expenses.iter() {
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

        // ---- 所得控除・税額控除マネージャー ----
        div { class: "card",
            h2 { "所得控除・税額控除の設定" }
            p { class: "muted",
                "確定申告書Bに反映する社会保険料控除、生命保険料控除、扶養控除、住宅ローン控除等の入力です。入力内容は自動計算・自動保存されます。"
            }

            // 1. 社会保険・小規模企業共済
            h3 { class: "subheading", "1. 社会保険料・小規模企業共済等掛金（iDeCo含む）" }
            div { class: "form-grid",
                div { class: "field",
                    label { "社会保険料控除（国民健康保険・国民年金等）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.social_insurance > 0 { format!("{}", deductions.social_insurance) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.social_insurance = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "小規模企業共済等掛金（共済・iDeCo掛金）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.small_enterprise_mutual_aid > 0 { format!("{}", deductions.small_enterprise_mutual_aid) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.small_enterprise_mutual_aid = val;
                            persist_quiet(app);
                        },
                    }
                }
            }

            // 2. 生命保険料控除 & 地震保険料控除
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-top: 1.25rem;",
                h3 { class: "subheading", style: "margin: 0;", "2. 生命保険料・地震保険料控除" }
                div {
                    span { class: "badge badge-income", style: "margin-right: 0.5rem;", "生保控除額: ¥{format_yen(d_detail.life_insurance)}（上限12万）" }
                    span { class: "badge badge-income", "地震控除額: ¥{format_yen(d_detail.earthquake_insurance)}（上限5万）" }
                }
            }
            p { class: "muted", style: "font-size: 0.82rem; margin: 0.25rem 0 0.75rem;", "支払保険料を入力すると、法定算式に基づき上限額（新制度各4万、合算最大12万/地震最大5万）を自動計算します。" }
            div { class: "form-grid",
                div { class: "field",
                    label { "新・一般生命保険料（平成24年以後）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.life_insurance_new_general > 0 { format!("{}", deductions.life_insurance_new_general) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.life_insurance_new_general = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "介護医療保険料（平成24年以後）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.life_insurance_new_care > 0 { format!("{}", deductions.life_insurance_new_care) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.life_insurance_new_care = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "新・個人年金保険料（平成24年以後）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.life_insurance_new_annuity > 0 { format!("{}", deductions.life_insurance_new_annuity) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.life_insurance_new_annuity = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "旧・一般生命保険料（平成23年以前）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.life_insurance_old_general > 0 { format!("{}", deductions.life_insurance_old_general) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.life_insurance_old_general = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "旧・個人年金保険料（平成23年以前）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.life_insurance_old_annuity > 0 { format!("{}", deductions.life_insurance_old_annuity) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.life_insurance_old_annuity = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "地震保険料" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.earthquake_insurance > 0 { format!("{}", deductions.earthquake_insurance) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.earthquake_insurance = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "旧長期損害保険料" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.earthquake_old_long_term > 0 { format!("{}", deductions.earthquake_old_long_term) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.earthquake_old_long_term = val;
                            persist_quiet(app);
                        },
                    }
                }
            }

            // 3. 医療費控除 & 寄附金控除（ふるさと納税）
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-top: 1.25rem;",
                h3 { class: "subheading", style: "margin: 0;", "3. 医療費控除 & 寄附金控除（ふるさと納税）" }
                div {
                    span { class: "badge badge-income", style: "margin-right: 0.5rem;", "医療費控除額: ¥{format_yen(d_detail.medical)}" }
                    span { class: "badge badge-income", "寄附金控除額: ¥{format_yen(d_detail.donation)}" }
                }
            }
            p { class: "muted", style: "font-size: 0.82rem; margin: 0.25rem 0 0.75rem;", "医療費控除は（支払額 - 保険補填額）から足切り基準額（10万円または総所得5%のいずれか低い額）を自動控除します。ふるさと納税は2,000円足切り後の控除額を算出します。" }
            div { class: "form-grid",
                div { class: "field",
                    label { "支払った医療費の総額" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.medical_expenses_paid > 0 { format!("{}", deductions.medical_expenses_paid) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.medical_expenses_paid = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "保険金等で補填される金額" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.medical_expenses_reimbursed > 0 { format!("{}", deductions.medical_expenses_reimbursed) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.medical_expenses_reimbursed = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "寄附金合計（ふるさと納税等）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.donations_paid > 0 { format!("{}", deductions.donations_paid) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.donations_paid = val;
                            persist_quiet(app);
                        },
                    }
                }
            }

            // 4. 配偶者控除・扶養控除
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-top: 1.25rem;",
                h3 { class: "subheading", style: "margin: 0;", "4. 配偶者控除 & 扶養控除" }
                div {
                    span { class: "badge badge-income", style: "margin-right: 0.5rem;", "配偶者控除: ¥{format_yen(d_detail.spouse)}" }
                    span { class: "badge badge-income", "扶養控除: ¥{format_yen(d_detail.dependents)}" }
                }
            }
            div { class: "form-grid",
                div { class: "field",
                    label { "配偶者控除区分" }
                    select {
                        value: match deductions.spouse_type {
                            SpouseDeductionType::None => "none",
                            SpouseDeductionType::General => "general",
                            SpouseDeductionType::Elderly => "elderly",
                            SpouseDeductionType::Special { .. } => "special",
                        },
                        onchange: move |evt| {
                            let t = match evt.value().as_str() {
                                "general" => SpouseDeductionType::General,
                                "elderly" => SpouseDeductionType::Elderly,
                                "special" => SpouseDeductionType::Special { amount: 380_000 },
                                _ => SpouseDeductionType::None,
                            };
                            app.write().deductions.spouse_type = t;
                            persist_quiet(app);
                        },
                        option { value: "none", "なし" }
                        option { value: "general", "配偶者控除（一般・70歳未満 38万円）" }
                        option { value: "elderly", "配偶者控除（老人・70歳以上 48万円）" }
                        option { value: "special", "配偶者特別控除（配偶者所得48万超〜133万以下）" }
                    }
                }
                if let SpouseDeductionType::Special { amount } = deductions.spouse_type {
                    div { class: "field",
                        label { "配偶者特別控除の基準額" }
                        input {
                            r#type: "number",
                            min: "0",
                            max: "380000",
                            value: "{amount}",
                            oninput: move |evt| {
                                let val = evt.value().parse::<i64>().unwrap_or(0);
                                app.write().deductions.spouse_type = SpouseDeductionType::Special { amount: val };
                                persist_quiet(app);
                            },
                        }
                    }
                }
                div { class: "field",
                    label { "一般扶養親族（16〜18歳、23〜69歳: 38万/人）" }
                    input {
                        r#type: "number",
                        min: "0",
                        value: "{deductions.dependents_general_count}",
                        oninput: move |evt| {
                            let val = evt.value().parse::<u32>().unwrap_or(0);
                            app.write().deductions.dependents_general_count = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "特定扶養親族（19〜22歳: 63万/人）" }
                    input {
                        r#type: "number",
                        min: "0",
                        value: "{deductions.dependents_specific_count}",
                        oninput: move |evt| {
                            let val = evt.value().parse::<u32>().unwrap_or(0);
                            app.write().deductions.dependents_specific_count = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "老人扶養・同居（70歳以上: 58万/人）" }
                    input {
                        r#type: "number",
                        min: "0",
                        value: "{deductions.dependents_elderly_living_count}",
                        oninput: move |evt| {
                            let val = evt.value().parse::<u32>().unwrap_or(0);
                            app.write().deductions.dependents_elderly_living_count = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "老人扶養・別居（70歳以上: 48万/人）" }
                    input {
                        r#type: "number",
                        min: "0",
                        value: "{deductions.dependents_elderly_other_count}",
                        oninput: move |evt| {
                            let val = evt.value().parse::<u32>().unwrap_or(0);
                            app.write().deductions.dependents_elderly_other_count = val;
                            persist_quiet(app);
                        },
                    }
                }
            }

            // 5. 障害者・ひとり親・寡婦・勤労学生 & 税額控除
            h3 { class: "subheading", "5. 障害者・ひとり親・勤労学生 & 税額控除" }
            div { class: "form-grid",
                div { class: "field",
                    label { "障害者控除合計額（普通27万/特別40万/同居特別75万）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.disability_deduction > 0 { format!("{}", deductions.disability_deduction) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.disability_deduction = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "寡婦・ひとり親控除" }
                    select {
                        value: match deductions.widow_or_single_parent {
                            WidowOrSingleParentType::None => "none",
                            WidowOrSingleParentType::Widow => "widow",
                            WidowOrSingleParentType::SingleParent => "single_parent",
                        },
                        onchange: move |evt| {
                            let t = match evt.value().as_str() {
                                "widow" => WidowOrSingleParentType::Widow,
                                "single_parent" => WidowOrSingleParentType::SingleParent,
                                _ => WidowOrSingleParentType::None,
                            };
                            app.write().deductions.widow_or_single_parent = t;
                            persist_quiet(app);
                        },
                        option { value: "none", "なし" }
                        option { value: "widow", "寡婦控除（27万円・所得500万以下）" }
                        option { value: "single_parent", "ひとり親控除（35万円・所得500万以下）" }
                    }
                }
                div { class: "field",
                    label { "勤労学生控除（27万円）" }
                    div { style: "display: flex; align-items: center; gap: 0.5rem; height: 38px;",
                        input {
                            r#type: "checkbox",
                            checked: deductions.working_student,
                            onchange: move |evt| {
                                app.write().deductions.working_student = evt.value() == "true";
                                persist_quiet(app);
                            },
                        }
                        span { style: "font-size: 0.9rem;", "勤労学生控除を適用する" }
                    }
                }
                div { class: "field",
                    label { "住宅借入金等特別控除（住宅ローン税額控除）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.housing_loan_tax_credit > 0 { format!("{}", deductions.housing_loan_tax_credit) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.housing_loan_tax_credit = val;
                            persist_quiet(app);
                        },
                    }
                }
                div { class: "field",
                    label { "その他の税額控除（配当控除等）" }
                    input {
                        r#type: "number",
                        min: "0",
                        placeholder: "0",
                        value: if deductions.other_tax_credits > 0 { format!("{}", deductions.other_tax_credits) } else { "".into() },
                        oninput: move |evt| {
                            let val = evt.value().parse::<i64>().unwrap_or(0);
                            app.write().deductions.other_tax_credits = val;
                            persist_quiet(app);
                        },
                    }
                }
            }
        }

        div { class: "card",
            h2 { "確定申告書B（所得税及び復興特別所得税の計算）" }
            p { class: "muted",
                "事業所得および各種所得控除・税額控除を反映した精密な所得税計算結果です。"
            }
            div { class: "report-grid",
                div { class: "stat",
                    div { class: "label", "所得金額合計（事業所得）" }
                    div { class: "value", "¥{format_yen(form_b.total_income)}" }
                }
                div { class: "stat",
                    div { class: "label", "所得控除合計（各種控除の合算）" }
                    div { class: "value", style: "color: #0369a1;", "¥{format_yen(form_b.total_deductions)}" }
                }
                div { class: "stat",
                    div { class: "label", "課税される所得金額（千円未満切捨て）" }
                    div { class: "value", "¥{format_yen(form_b.taxable_income)}" }
                }
                div { class: "stat",
                    div { class: "label", "算出所得税額（税額控除前）" }
                    div { class: "value", "¥{format_yen(form_b.tax_before_credits)}" }
                }
                div { class: "stat",
                    div { class: "label", "税額控除合計（住宅ローン控除等）" }
                    div { class: "value", style: "color: #b45309;", "¥{format_yen(form_b.total_tax_credits)}" }
                }
                div { class: "stat",
                    div { class: "label", "基準所得税額（差引税額）" }
                    div { class: "value", "¥{format_yen(form_b.income_tax)}" }
                }
                div { class: "stat",
                    div { class: "label", "復興特別所得税額（×2.1%）" }
                    div { class: "value", "¥{format_yen(form_b.reconstruction_surtax)}" }
                }
                div { class: "stat",
                    div { class: "label", "申告納税額（所得税及び復興特別所得税）" }
                    div { class: "value", style: "color: var(--primary); font-size: 1.35rem;", "¥{format_yen(form_b.total_income_tax)}" }
                }
            }

            h3 { class: "subheading", "所得控除の内訳" }
            table { class: "ledger",
                thead {
                    tr {
                        th { "控除項目" }
                        th { "控除額" }
                    }
                }
                tbody {
                    tr {
                        td { "基礎控除" }
                        td { class: "amount", "¥{format_yen(d_detail.basic)}" }
                    }
                    if d_detail.social_insurance > 0 {
                        tr {
                            td { "社会保険料控除" }
                            td { class: "amount", "¥{format_yen(d_detail.social_insurance)}" }
                        }
                    }
                    if d_detail.small_enterprise_mutual_aid > 0 {
                        tr {
                            td { "小規模企業共済等掛金控除（iDeCo含む）" }
                            td { class: "amount", "¥{format_yen(d_detail.small_enterprise_mutual_aid)}" }
                        }
                    }
                    if d_detail.life_insurance > 0 {
                        tr {
                            td { "生命保険料控除" }
                            td { class: "amount", "¥{format_yen(d_detail.life_insurance)}" }
                        }
                    }
                    if d_detail.earthquake_insurance > 0 {
                        tr {
                            td { "地震保険料控除" }
                            td { class: "amount", "¥{format_yen(d_detail.earthquake_insurance)}" }
                        }
                    }
                    if d_detail.medical > 0 {
                        tr {
                            td { "医療費控除" }
                            td { class: "amount", "¥{format_yen(d_detail.medical)}" }
                        }
                    }
                    if d_detail.donation > 0 {
                        tr {
                            td { "寄附金控除（ふるさと納税等）" }
                            td { class: "amount", "¥{format_yen(d_detail.donation)}" }
                        }
                    }
                    if d_detail.spouse > 0 {
                        tr {
                            td { "配偶者控除・配偶者特別控除" }
                            td { class: "amount", "¥{format_yen(d_detail.spouse)}" }
                        }
                    }
                    if d_detail.dependents > 0 {
                        tr {
                            td { "扶養控除" }
                            td { class: "amount", "¥{format_yen(d_detail.dependents)}" }
                        }
                    }
                    if d_detail.disability > 0 {
                        tr {
                            td { "障害者控除" }
                            td { class: "amount", "¥{format_yen(d_detail.disability)}" }
                        }
                    }
                    if d_detail.widow_or_single_parent > 0 {
                        tr {
                            td { "寡婦・ひとり親控除" }
                            td { class: "amount", "¥{format_yen(d_detail.widow_or_single_parent)}" }
                        }
                    }
                    if d_detail.working_student > 0 {
                        tr {
                            td { "勤労学生控除" }
                            td { class: "amount", "¥{format_yen(d_detail.working_student)}" }
                        }
                    }
                    tr {
                        td { strong { "所得控除 合計" } }
                        td { class: "amount", strong { "¥{format_yen(form_b.total_deductions)}" } }
                    }
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
    let report = build_tax_report_with_deductions(
        &state.entries,
        state.fiscal_year,
        state.special_deduction,
        &state.deductions,
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

