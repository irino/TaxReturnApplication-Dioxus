# Implementation Plan: Custom Tax Return Software (Dioxus Desktop)

**Reference:** `AGENTS.md`  
**Status:** Greenfield — repository contains only `AGENTS.md`  
**Target platforms:** Windows, macOS (single-binary desktop delivery)

---

## 1. Goals

Replace a legacy Excel-macro tax tool with a pure-Rust desktop app that:

1. Lets users enter and manage financial ledgers (double-entry style, simplified UX).
2. Generates official Japanese tax PDFs (国税庁 layouts) via template overlay.
3. Ships as a single executable with no JS/TS in the UI layer.

This plan implements the architecture and milestones in `AGENTS.md`, then extends them with concrete data models, dependencies, file layout, and follow-on phases (tax aggregation + PDF).

---

## 2. Current State

| Item | Status |
|------|--------|
| Project bootstrap | Completed (Pure Rust / Dioxus Desktop architecture) |
| `src/` modules | Completed (`core/`, `storage/`, `pdf/`, `ui/`, `main.rs`, `lib.rs`) |
| Core journal models | Completed (`JournalEntry`, `AccountCode`, `validate_entry`, unit tests) |
| UI shell / ledger | Completed (Reactive input form with Add & Edit, date range filter, sort order toggle, stats summary) |
| Storage | Completed (OS project data dirs, per-year JSON atomic save/load, auto-recovery) |
| Tax calc / PDF | Completed (Blue Return P&L + special deduction 0/100k/650k, Form B brackets, PDF engine with JSON field maps) |
| Toolchain & Tests | 27 unit tests passing, `cargo check --features ui` clean |

---

## 3. Technology Decisions (from AGENTS.md)

| Area | Choice | Notes |
|------|--------|--------|
| App shell | Dioxus Desktop | Pure Rust RSX; single binary |
| UI pattern | Input form + filterable ledger table | **No** Excel-like grid |
| Core logic | Pure functions in `src/core/` | No UI deps; unit-tested |
| Persistence (v1) | `serde_json` + `directories` | One JSON file per fiscal year |
| Persistence (optional later) | `rusqlite` | Only if JSON becomes limiting |
| PDF engine (later) | `lopdf` + `include_bytes!` templates | Coordinate maps externalizable (JSON/structs) |

**Constraint:** Do not inject JavaScript/TypeScript. All UI is Dioxus RSX.

---

## 4. Target Directory Layout

```
TaxReturnApplication-Dioxus/
├── AGENTS.md
├── Cargo.toml
├── Dioxus.toml                  # if required by dx bootstrap
├── assets/                      # CSS / icons if any
├── templates/                   # blank NTA PDFs (later; embedded at build)
│   └── ...
├── src/
│   ├── main.rs                  # entry + window orchestration
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── layout.rs            # shell, sidebar, navigation
│   │   ├── ledger.rs            # input form + journal list
│   │   └── report.rs            # tax config + PDF preview (later stub OK)
│   ├── core/
│   │   ├── mod.rs
│   │   ├── journal.rs           # JournalEntry, AccountCode, validation
│   │   └── tax_calc.rs          # 青色申告決算書 / 確定申告書B aggregations
│   └── storage/
│       ├── mod.rs
│       └── paths.rs             # app data dirs via directories
│       └── json_store.rs        # load/save fiscal-year files
└── tests/                       # optional integration tests
```

Strict separation: **UI → signals/state → core (pure) → storage**. Core never imports UI.

---

## 5. Core Data Models (Phase 1)

Define in `src/core/journal.rs` (serde-friendly, UI-agnostic).

### 5.1 Account chart

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum AccountCode {
    // Assets
    Cash,                    // 現金
    Bank,                    // 普通預金
    AccountsReceivable,      // 売掛金
    // Liabilities
    AccountsPayable,         // 買掛金
    // Equity / owner
    Capital,                 // 元入金
    Drawings,                // 事業主貸
    OwnerContribution,       // 事業主借
    // Income
    Sales,                   // 売上高
    // Expenses (expandable)
    CostOfSales,             // 仕入高
    Travel,                  // 旅費交通費
    Communication,           // 通信費
    Supplies,                // 消耗品費
    Rent,                    // 地代家賃
    Utilities,               // 水道光熱費
    Miscellaneous,           // 雑費
    // Extensible: add as needed for blue-return categories
}

impl AccountCode {
    pub fn label_ja(&self) -> &'static str { /* ... */ }
    pub fn side_hint(&self) -> AccountNature { /* Asset/Liability/Equity/Income/Expense */ }
}
```

Start with a small fixed chart of accounts; make it easy to extend without breaking saved JSON (use stable string discriminants or versioned serde).

### 5.2 Journal entry

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JournalEntry {
    pub id: String,              // UUID or nanoid
    pub date: NaiveDate,         // chrono
    pub debit: AccountCode,
    pub credit: AccountCode,
    pub amount: i64,             // yen integers; avoid f64
    pub description: String,
    pub created_at: DateTime<Local>,
}

pub fn validate_entry(entry: &JournalEntry) -> Result<(), JournalError> {
    // amount > 0, debit != credit, date in fiscal year, etc.
}
```

### 5.3 Fiscal year document

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerDocument {
    pub version: u32,            // schema version for migrations
    pub fiscal_year: i32,        // e.g. 2025 → 令和7年分
    pub entries: Vec<JournalEntry>,
}
```

### 5.4 App state (UI layer only)

```rust
// In main or a state module; held in Dioxus signals
struct AppState {
    fiscal_year: i32,
    entries: Vec<JournalEntry>,
    dirty: bool,
}
```

Use `use_signal` / shared signal context for reactive updates.

---

## 6. Phased Implementation

### Phase 0: Toolchain & Bootstrap

1. Install Dioxus CLI if missing (`cargo install dioxus-cli` / project-recommended version).
2. Bootstrap desktop app: `dx new --platform desktop` (or equivalent current CLI) in the repo root, **preserving** `AGENTS.md`.
3. Verify `dx serve` / `cargo build` produces a runnable desktop window.
4. Wire `Cargo.toml` dependencies:

| Crate | Purpose |
|-------|---------|
| `dioxus` (+ desktop feature) | UI |
| `serde`, `serde_json` | Persistence |
| `chrono` | Dates |
| `directories` | OS app data paths |
| `uuid` (or similar) | Entry IDs |
| `thiserror` / `anyhow` | Errors (prefer `thiserror` in core) |

Later phases add: `lopdf` (PDF), optional `rusqlite`.

5. Create empty module tree: `ui/`, `core/`, `storage/` with `mod.rs` files and re-exports from `main.rs`.

**Exit criteria:** Blank desktop window launches; project compiles.

---

### Phase 1: Environment & State Foundation

Per AGENTS.md:

1. Implement `JournalEntry`, `AccountCode`, `LedgerDocument`, `validate_entry` in `src/core/journal.rs`.
2. Unit tests for validation (zero amount, same debit/credit, etc.).
3. App shell in `main.rs` + `ui/layout.rs`:
   - Window title / basic chrome
   - Sidebar nav placeholders: **仕訳** (Ledger), **帳票** (Report)
4. Global state via Dioxus signals:
   - `entries: Signal<Vec<JournalEntry>>`
   - `fiscal_year: Signal<i32>`
5. Stub `ui/ledger.rs` and `ui/report.rs` so navigation switches views.

**Exit criteria:** Models tested; shell navigable; state exists (even if empty list).

---

### Phase 2: Ledger UI Implementation

**UI architecture:** Modern form + filterable list — not a spreadsheet grid.

#### 2.1 Input form (`ui/ledger.rs`)

Fields:

| Field | Control |
|-------|---------|
| Date | date input / text with parse |
| Debit account | select from `AccountCode` (Japanese labels) |
| Credit account | select |
| Amount | integer yen |
| Description | text |

Actions:

- **登録** — validate via `core::journal::validate_entry`, push into signal, clear form (or keep date).
- Show inline validation errors (pure core errors mapped to UI text).

#### 2.2 Ledger list table

Columns: 日付 | 借方 | 貸方 | 金額 | 摘要 | (削除)

Features (v1 minimum):

- Newest-first or date-sorted list
- Delete entry by id
- Optional simple filter: text match on description, account filter, date range

Reactive: list re-renders from `entries` signal on every insert/delete.

#### 2.3 Layout polish

- Keyboard-friendly tab order
- Yen formatting (`1,234`)
- Empty state message when no entries

**Exit criteria:** User can add, view, filter, and delete journal entries in-session (memory only).

---

### Phase 3: Storage Integration

Implement `src/storage/`:

1. **`paths.rs`**
   - Use `directories::ProjectDirs` with a stable qualifier (e.g. org `local`, app `TaxReturnApplication`).
   - Path pattern: `{data_dir}/ledgers/{fiscal_year}.json`

2. **`json_store.rs`**
   - `load_year(year: i32) -> Result<LedgerDocument>`
   - `save_year(doc: &LedgerDocument) -> Result<()>`
   - Create parent dirs if missing
   - Atomic write preferred (write temp + rename)

3. **App lifecycle wiring**
   - On startup: load current (or last) fiscal year into signals; if missing, empty document.
   - On change: either auto-save debounce or explicit **保存** button (recommend auto-save on successful insert/delete for Phase 3 simplicity, plus save-on-exit if feasible).
   - Surface load/save errors in UI without panicking.

4. **Schema version**
   - Write `version: 1` on all files; keep a single place for future migrations.

**Exit criteria:** Restart app → previous entries restored for that fiscal year.

---

### Phase 4: Tax Aggregation Engine (core)

Not fully specified as “initial” in AGENTS.md, but required by architecture (`tax_calc.rs`).

1. Pure aggregations from `&[JournalEntry]`:
   - Account balances (debit − credit by nature)
   - Income / expense totals mapped to 青色申告決算書 line items
   - Preliminary 所得金額 inputs for 確定申告書B
2. Types like `BlueReturnSummary`, `FormBSummary` with yen `i64` fields.
3. Extensive `#[test]` coverage with fixture journals (known totals).
4. Keep pure: `fn aggregate_blue_return(entries: &[JournalEntry], year: i32) -> BlueReturnSummary`.

**Exit criteria:** Deterministic, tested numbers from sample ledgers; no UI dependency.

---

### Phase 5: PDF Generation (Target Sheet Engine)

1. Obtain/embed blank official PDFs under `templates/` (respect licensing/redistribution of 国税庁 forms).
2. `include_bytes!` into binary.
3. Coordinate map (JSON or Rust const maps): field name → (page, x, y, font_size).
4. `lopdf` overlay pipeline:
   - Load template bytes
   - Draw text/numbers at coordinates from `tax_calc` output
   - Write PDF to user-chosen path or app data `exports/`
5. `ui/report.rs`:
   - Select form type (青色申告決算書 / 確定申告書B)
   - Preview summary numbers (text table is enough before true PDF preview)
   - **PDF出力** button

**Exit criteria:** Generated PDF opens with correct sample values at correct positions for at least one form page.

---

## 7. Recommended Implementation Order (PR-sized chunks)

| PR | Scope | Depends on | Status |
|----|--------|------------|--------|
| PR1 | Bootstrap Dioxus desktop + module skeleton + deps | — | [x] Completed |
| PR2 | `core/journal` models + validation + unit tests | PR1 | [x] Completed |
| PR3 | Layout shell + signal state + nav stubs | PR1 | [x] Completed |
| PR4 | Ledger form + edit mode + list + delete + filter (text/account/dates) + sort | PR2, PR3 | [x] Completed |
| PR5 | `storage` JSON load/save + startup restore | PR2, PR4 | [x] Completed |
| PR6 | `tax_calc` aggregations + brackets + tests | PR2 | [x] Completed |
| PR7 | Report UI summary + PDF overlay + JSON field maps + export directory launcher | PR5, PR6 | [x] Completed |

Phases 1–3 in AGENTS.md map to PR1–PR5. Phases 4–5 (PR6–PR7) complete the product goals.

---

## 8. Testing Strategy

| Layer | Approach |
|-------|----------|
| `core/journal` | Unit tests: validation, account labels, serde round-trip |
| `core/tax_calc` | Unit tests: fixture ledgers → expected totals |
| `storage` | Unit/integration: temp dir load/save/migrate |
| UI | Manual smoke via `dx serve` / desktop run for Phases 1–3 |

Rule from AGENTS.md: **all accounting formulas and aggregations must have unit tests**; core functions stay pure and side-effect free.

---

## 9. Non-Goals (for Phases 1–3)

- Full Excel-like grid / cell editing
- Multi-user / cloud sync
- e-Tax XML submission
- Complete chart of accounts for every industry
- Perfect visual PDF parity on day one (Phase 5 MVP first)

---

## 10. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Dioxus CLI/version mismatch | Pin dioxus + dx versions in docs; verify bootstrap early |
| Floating-point money bugs | Use `i64` yen only |
| PDF coordinate drift yearly | Externalize field maps; re-measure per year |
| Form redistribution rights | Confirm 国税庁 template usage; document source |
| Scope creep on accounts | Ship minimal chart; extend by enum + label map |
| Auto-save data loss | Atomic writes; schema version; optional backup on save |

---

## 11. Definition of Done (MVP = Phases 1–3)

- [x] Desktop app builds as single binary on target OS
- [x] User can enter journal entries (date, debit, credit, amount, description) and edit existing entries
- [x] List is filterable (text, account, date-range) and sortable (newest/oldest) and updates reactively
- [x] Data persists per fiscal year under OS app data dir
- [x] Core journal logic and tax calculation covered by unit tests (27 tests passing)
- [x] No JS/TS; modules follow AGENTS.md layout
- [x] Pure core (no UI imports in `src/core/`)

**Product-complete stretch (Phases 4–5):**
- [x] Blue-return P&L aggregation + special deductions (0 / 100k / 650k)
- [x] Form B income-tax estimate with brackets and deduction phase-outs
- [x] NTA PDF layout export with customizable JSON field maps and export folder shortcut

---

## 12. Immediate Next Steps (when executing)

1. Install/pin Dioxus CLI and create desktop project without deleting `AGENTS.md`.
2. Add module skeleton and dependencies.
3. Implement `core/journal.rs` + tests.
4. Build layout + ledger form/list.
5. Add JSON storage and startup load.

---

## 13. Open Points (defaults assumed unless user overrides)

| Topic | Default assumption |
|-------|-------------------|
| Persistence | JSON files first (`serde_json`), not SQLite |
| Fiscal year default | Calendar year of “today” (Japan local) |
| Currency | JPY integer only |
| Language (UI) | Japanese labels for accounts/forms; code identifiers English |
| Save UX | Auto-save after successful mutations |
| ID generation | UUID v4 strings |
| Phase 4–5 timing | After MVP ledger works end-to-end |

---

*Plan derived from `AGENTS.md` for TaxReturnApplication-Dioxus. Ready for execution starting at Phase 0/1.*
