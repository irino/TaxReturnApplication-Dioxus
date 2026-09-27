# Tax Return Application (Dioxus)

Custom Japanese tax-return desktop app built with a pure Rust / Dioxus stack.

See `AGENTS.md` for product goals and `PLAN.md` for the implementation roadmap.

## Features (current)

- Double-entry style journal input (date, debit, credit, amount, description) with interactive edit and delete
- Filterable and sortable ledger list (text match, account select, date range, date asc/desc toggle, real-time total amount sum)
- Per fiscal-year JSON persistence under the OS app data directory with atomic writes
- Blue-return P&L aggregation + special deduction (0 / 10万 / 65万) + balance sheet balances
- Form B style income-tax estimate (brackets, basic deduction phase-outs, reconstruction surtax)
- PDF export (blue return + Form B summary sheets) with external JSON coordinate overrides and folder quick-launch

## Develop

```bash
# Unit tests (core + storage; no GUI system libs required)
cargo test --lib --no-default-features

# Desktop app (requires WebKitGTK / GTK dev packages on Linux)
cargo run --features ui
```

### Linux system packages

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

## Data location

- Ledgers: `{app_data}/TaxReturnApplication/ledgers/{year}.json`
- PDF exports: `{app_data}/TaxReturnApplication/exports/`

## Japanese fonts for PDF

PDF に日本語を埋め込むには TTF/OTF が必要です（TTC は非対応の場合あり）。

```bash
# Debian/Ubuntu 例
sudo apt install fonts-ipafont-gothic

# または任意フォントを指定
export TAX_RETURN_FONT=/path/to/NotoSansJP-Regular.otf
```

## Module layout

- `src/core/` — pure accounting / tax logic and unit tests
- `src/pdf/` — field maps + PDF engine
- `src/storage/` — paths + JSON load/save
- `src/ui/` — Dioxus RSX components
- `src/main.rs` — desktop entry / window
