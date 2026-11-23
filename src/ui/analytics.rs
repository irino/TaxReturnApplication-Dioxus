//! Business Analytics & Management Dashboard page (経営分析・ダッシュボード).
//!
//! Provides visual financial reports, monthly P&L trend graphs, cashflow monitoring,
//! and KPI metrics rendered cleanly via SVG and reactive Dioxus components.

use crate::AppState;
use dioxus::prelude::*;
use tax_return_application::core::analytics::{calculate_analytics, AnalyticsSummary};
use tax_return_application::core::journal::format_yen;

#[component]
pub fn AnalyticsPage() -> Element {
    let app = use_context::<Signal<AppState>>();
    let doc_snapshot = app.read().to_document();
    let summary: AnalyticsSummary = calculate_analytics(&doc_snapshot);
    let fiscal_year = summary.fiscal_year;
    let kpi = &summary.kpis;

    // グラフ描画用の最大値計算（スケール調整）
    let max_monthly_val = summary
        .monthly_metrics
        .iter()
        .map(|m| m.revenue.max(m.expenses))
        .max()
        .unwrap_or(1)
        .max(100_000);

    rsx! {
        h1 { class: "page-title", "{fiscal_year}年 経営分析ダッシュボード" }

        // ---- KPI サマリーカード群 ----
        div { class: "grid-cards", style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 1.5rem;",
            div { class: "card", style: "margin: 0; border-top: 4px solid #2563eb;",
                span { class: "muted", style: "font-size: 0.85rem; font-weight: 600;", "年間売上高" }
                h3 { style: "margin: 0.3rem 0; font-size: 1.5rem; color: #1e3a8a;", "¥{format_yen(kpi.annual_revenue)}" }
                span { style: "font-size: 0.8rem; color: var(--muted);", "粗利率: {kpi.gross_margin_percent}%" }
            }
            div { class: "card", style: "margin: 0; border-top: 4px solid #ef4444;",
                span { class: "muted", style: "font-size: 0.85rem; font-weight: 600;", "年間総経費" }
                h3 { style: "margin: 0.3rem 0; font-size: 1.5rem; color: #991b1b;", "¥{format_yen(kpi.annual_expenses)}" }
                if let Some((name, amt)) = &kpi.largest_expense {
                    span { style: "font-size: 0.8rem; color: var(--muted);", "最大: {name} (¥{format_yen(*amt)})" }
                } else {
                    span { style: "font-size: 0.8rem; color: var(--muted);", "経費なし" }
                }
            }
            div { class: "card", style: "margin: 0; border-top: 4px solid #10b981;",
                span { class: "muted", style: "font-size: 0.85rem; font-weight: 600;", "青色申告前所得 (純利益)" }
                h3 {
                    style: if kpi.net_business_income >= 0 { "margin: 0.3rem 0; font-size: 1.5rem; color: #065f46;" } else { "margin: 0.3rem 0; font-size: 1.5rem; color: #ef4444;" },
                    "¥{format_yen(kpi.net_business_income)}"
                }
                span { style: "font-size: 0.8rem; color: var(--muted);", "営業利益率: {kpi.operating_margin_percent}%" }
            }
            div { class: "card", style: "margin: 0; border-top: 4px solid #f59e0b;",
                span { class: "muted", style: "font-size: 0.85rem; font-weight: 600;", "期末現預金残高" }
                h3 { style: "margin: 0.3rem 0; font-size: 1.5rem; color: #b45309;", "¥{format_yen(kpi.cash_and_bank_balance)}" }
                span { style: "font-size: 0.8rem; color: var(--muted);", "売掛金: ¥{format_yen(kpi.accounts_receivable)}" }
            }
        }

        // ---- 月次損益推移グラフ (SVG) ----
        div { class: "card", style: "margin-bottom: 1.5rem;",
            h2 { "月次売上・経費・営業利益の推移" }
            p { class: "muted", "青棒: 売上高、赤棒: 経費、緑折れ線: 営業所得" }

            div { style: "width: 100%; overflow-x: auto; padding: 0.5rem 0;",
                svg {
                    view_box: "0 0 800 240",
                    style: "width: 100%; min-width: 600px; height: 240px; background: #fafafa; border-radius: 6px; border: 1px solid #e5e7eb;",

                    // グリッド背景横線
                    line { x1: "40", y1: "30", x2: "780", y2: "30", stroke: "#e5e7eb", stroke_dasharray: "3,3" }
                    line { x1: "40", y1: "80", x2: "780", y2: "80", stroke: "#e5e7eb", stroke_dasharray: "3,3" }
                    line { x1: "40", y1: "130", x2: "780", y2: "130", stroke: "#e5e7eb", stroke_dasharray: "3,3" }
                    line { x1: "40", y1: "180", x2: "780", y2: "180", stroke: "#9ca3af", stroke_width: "1.5" }

                    // Y軸ラベル
                    text { x: "5", y: "35", font_size: "10", fill: "#9ca3af", "¥{format_yen(max_monthly_val)}" }
                    text { x: "5", y: "105", font_size: "10", fill: "#9ca3af", "¥{format_yen(max_monthly_val / 2)}" }
                    text { x: "15", y: "184", font_size: "10", fill: "#9ca3af", "0" }

                    // 各月の棒グラフ
                    for (i, m) in summary.monthly_metrics.iter().enumerate() {
                        {
                            let base_x = 55.0 + (i as f64) * 60.0;
                            let rev_height = if max_monthly_val > 0 { (m.revenue as f64 / max_monthly_val as f64) * 140.0 } else { 0.0 };
                            let exp_height = if max_monthly_val > 0 { (m.expenses as f64 / max_monthly_val as f64) * 140.0 } else { 0.0 };
                            let inc_y = if max_monthly_val > 0 {
                                180.0 - ((m.net_income as f64 / max_monthly_val as f64) * 140.0)
                            } else {
                                180.0
                            };

                            rsx! {
                                g { key: "{m.month}",
                                    // 売上棒（青）
                                    rect {
                                        x: "{base_x}",
                                        y: "{180.0 - rev_height}",
                                        width: "18",
                                        height: "{rev_height}",
                                        fill: "#3b82f6",
                                        rx: "3",
                                    }
                                    // 経費棒（赤）
                                    rect {
                                        x: "{base_x + 20.0}",
                                        y: "{180.0 - exp_height}",
                                        width: "18",
                                        height: "{exp_height}",
                                        fill: "#ef4444",
                                        rx: "3",
                                    }
                                    // 利益ポイント（緑ドット）
                                    circle {
                                        cx: "{base_x + 19.0}",
                                        cy: "{inc_y.clamp(20.0, 210.0)}",
                                        r: "4",
                                        fill: "#10b981",
                                    }
                                    // 月ラベル
                                    text {
                                        x: "{base_x + 10.0}",
                                        y: "202",
                                        font_size: "11",
                                        fill: "#4b5563",
                                        font_weight: "bold",
                                        "{m.month}月"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ---- 経費構成比率 & キャッシュフローの2カラム ----
        div { style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(360px, 1fr)); gap: 1.5rem; margin-bottom: 1.5rem;",
            // 経費構成比ランキング
            div { class: "card", style: "margin: 0;",
                h2 { "経費科目別 構成比率ランキング" }
                p { class: "muted", "全経費（¥{format_yen(kpi.annual_expenses)}）に占める割合上位" }

                div { style: "display: flex; flex-direction: column; gap: 0.75rem; margin-top: 0.8rem;",
                    for (rank, share) in summary.expense_shares.iter().take(6).enumerate() {
                        div { key: "{share.account_name}", style: "display: flex; flex-direction: column; gap: 0.25rem;",
                            div { style: "display: flex; justify-content: space-between; font-size: 0.9rem;",
                                span {
                                    strong { "#{rank + 1} {share.account_name}" }
                                }
                                span {
                                    "¥{format_yen(share.amount)} "
                                    strong { style: "color: #2563eb; margin-left: 0.4rem;", "{share.percentage}%" }
                                }
                            }
                            // プログレスバー
                            div { style: "width: 100%; height: 8px; background: #e5e7eb; border-radius: 4px; overflow: hidden;",
                                div {
                                    style: "width: {share.percentage}%; height: 100%; background: #3b82f6; border-radius: 4px;",
                                }
                            }
                        }
                    }
                    if summary.expense_shares.is_empty() {
                        p { class: "muted", "経費データはまだ登録されていません。" }
                    }
                }
            }

            // 四半期別サマリー
            div { class: "card", style: "margin: 0;",
                h2 { "四半期別 業績推移" }
                p { class: "muted", "3ヶ月ごとの売上高と純利益の推移" }

                div { class: "table-container", style: "margin-top: 0.8rem;",
                    table { class: "data-table",
                        thead {
                            tr {
                                th { "四半期" }
                                th { style: "text-align: right;", "売上高" }
                                th { style: "text-align: right;", "営業所得" }
                                th { style: "text-align: right;", "利益率" }
                            }
                        }
                        tbody {
                            for (q_idx, q_name) in ["第1四半期 (1〜3月)", "第2四半期 (4〜6月)", "第3四半期 (7〜9月)", "第4四半期 (10〜12月)"].iter().enumerate() {
                                {
                                    let rev = summary.quarterly_revenue[q_idx];
                                    let inc = summary.quarterly_income[q_idx];
                                    let margin = if rev > 0 { ((inc as f64 / rev as f64) * 100.0 * 10.0).round() / 10.0 } else { 0.0 };
                                    rsx! {
                                        tr { key: "{q_name}",
                                            td { strong { "{q_name}" } }
                                            td { style: "text-align: right;", "¥{format_yen(rev)}" }
                                            td {
                                                style: if inc >= 0 { "text-align: right; color: #10b981;" } else { "text-align: right; color: #ef4444;" },
                                                "¥{format_yen(inc)}"
                                            }
                                            td { style: "text-align: right;", "{margin}%" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ---- 月次詳細データテーブル ----
        div { class: "card",
            h2 { "月次詳細財務テーブル" }
            div { class: "table-container",
                table { class: "data-table",
                    thead {
                        tr {
                            th { "月" }
                            th { style: "text-align: right;", "売上高" }
                            th { style: "text-align: right;", "売上原価" }
                            th { style: "text-align: right;", "売上総利益" }
                            th { style: "text-align: right;", "経費計" }
                            th { style: "text-align: right;", "営業所得 (純利益)" }
                            th { style: "text-align: right;", "現預金流入" }
                            th { style: "text-align: right;", "現預金流出" }
                            th { style: "text-align: right;", "正味資金増減" }
                        }
                    }
                    tbody {
                        for m in summary.monthly_metrics.iter() {
                            tr { key: "{m.month}",
                                td { strong { "{m.month}月" } }
                                td { style: "text-align: right;", "¥{format_yen(m.revenue)}" }
                                td { style: "text-align: right;", "¥{format_yen(m.cogs)}" }
                                td { style: "text-align: right;", "¥{format_yen(m.gross_profit)}" }
                                td { style: "text-align: right;", "¥{format_yen(m.expenses)}" }
                                td {
                                    style: if m.net_income >= 0 { "text-align: right; color: #10b981; font-weight: bold;" } else { "text-align: right; color: #ef4444; font-weight: bold;" },
                                    "¥{format_yen(m.net_income)}"
                                }
                                td { style: "text-align: right;", "+¥{format_yen(m.cash_inflow)}" }
                                td { style: "text-align: right;", "▲¥{format_yen(m.cash_outflow)}" }
                                td {
                                    style: if m.cash_net >= 0 { "text-align: right; color: #10b981;" } else { "text-align: right; color: #ef4444;" },
                                    if m.cash_net >= 0 { "+¥{format_yen(m.cash_net)}" } else { "▲¥{format_yen(m.cash_net.abs())}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
