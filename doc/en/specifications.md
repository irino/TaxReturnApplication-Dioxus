# Tax Return Application Specifications (Current Features)

This document provides a comprehensive specification of the **currently implemented features, operational workflows, data models, and specifications** of the Rust / Dioxus-based desktop tax return application (`TaxReturnApplication-Dioxus`) at version 0.1.0.

---

## 1. Product Overview

### 1.1 Core Principles
- **Pure Rust Stack**: Single-binary desktop application built with Dioxus (Desktop). Zero JavaScript / TypeScript in the UI layer; 100% Rust RSX syntax.
- **Privacy & Complete Locality**: Runs entirely offline on the user's machine. Financial ledgers are stored in local JSON files and never sent to cloud servers.
- **Modern Operational Flow**: Avoids fragile, bloated Excel-like web grids. Instead, it utilizes a **"Modern Input Form + Filterable / Sortable Ledger Table"** pattern for rapid, fluid bookkeeping.
- **Legacy Asset Compatibility**: Includes an automated high-throughput import engine for legacy Excel-macro tax tools using 2-row header CSV files (Shift_JIS / UTF-8).
- **Automated Tax Calculation & PDF Export**: Automatically aggregates Blue Return Financial Statements (青色申告決算書) and Tax Return Form B (確定申告書B) estimates, generating printable A4 PDF summaries.

---

## 2. UI Layout & Navigation

The application uses a persistent sidebar navigation shell.

```
+-----------------------------------------------------------------------------------+
|  Tax Return Application (Desktop Window)                                          |
+-------------------+---------------------------------------------------------------+
| [Sidebar Nav]     | [Main Content Area]                                           |
| 確定申告ソフト    |                                                               |
|   仕訳/帳票/決算  | 1. Ledger Page (LedgerPage)                                   |
|                   |    - Fiscal year selector / Legacy Excel CSV import box       |
| [ 仕訳 (Active) ] |    - Journal entry form (Create / Edit with Sub-Accounts)     |
| [ 帳票          ] |    - Filter & sort bar with real-time statistics              |
| [ 決算・設定    ] |    - Reactive journal entry table with sub-account badges     |
|                   |                                                               |
|                   | 2. Report Page (ReportPage)                                   |
|                   |    - Taxpayer personal & business profile form                |
|                   |    - Blue return special deduction tier selector              |
|                   |    - Blue return P&L preview & all expense itemized breakdown |
|                   |    - Form B income tax estimate preview                       |
|                   |    - PDF export triggers & export folder quick launcher       |
|                   |    - Balance by account summary                               |
|                   |                                                               |
|                   | 3. Closing & Settings Page (ClosingPage)                      |
|                   |    - Chart of accounts & sub-account register (custom accounts|
|                   |    - Home-office expense allocation (business ratio %, apply) |
|                   |    - Fiscal year rollover (B/S carry-forward, opening entries)|
+-------------------+---------------------------------------------------------------+
```

---

## 3. Journal & Ledger Management (LedgerPage)

### 3.1 Fiscal Year Switching
- **Supported Years**: Instant switching between `current_year - 3` to `current_year + 1` via dropdown.
- **Switching Behavior**:
  - Automatically loads `{fiscal_year}.json` from local storage.
  - Resets any active edit state and CSV parsing reports.

### 3.2 Journal Entry Model (`JournalEntry`)
Each journal entry follows standard double-entry bookkeeping with a single debit and single credit account.

| Field | Type / Format | Required | Description & Validation Rules |
|---|---|:---:|---|
| **ID** | `String` (UUID v4) | Yes | Unique identifier per entry. Generated automatically. |
| **Date (`date`)** | `NaiveDate` (YYYY-MM-DD) | Yes | Transaction date. **Must fall within the selected fiscal year (Jan 1 – Dec 31)**. |
| **Debit (`debit`)** | `AccountCode` | Yes | Debit account code (standard or custom). |
| **Debit Sub (`debit_sub`)** | `Option<String>` | Optional | Sub-account identifier (e.g. bank name, client name). |
| **Credit (`credit`)** | `AccountCode` | Yes | Credit account code. **Must not be identical to debit account, unless transferring between distinct sub-accounts**. |
| **Credit Sub (`credit_sub`)** | `Option<String>` | Optional | Sub-account identifier (e.g. bank name, credit card). |
| **Amount (`amount`)** | `i64` (Yen integer) | Yes | Transaction amount in JPY. Stored as integer to eliminate floating-point drift. **Must be positive (>= 1)**. |
| **Description (`description`)** | `String` | Yes | Memo or description. **Cannot be empty or whitespace only**. |
| **Created At (`created_at`)** | `DateTime<Local>` | Yes | Creation timestamp, used as a tie-breaker for deterministic sorting. |

### 3.3 Create, Edit, and Delete Lifecycle
1. **Creation**:
   - Fill in date, debit account, credit account, amount, and description, then click "登録" (Register).
   - Validation failures are displayed inline below the form in Japanese.
   - Upon successful insertion, amount and description are cleared while preserving the date for rapid subsequent entries.
   - Triggers an **immediate atomic auto-save** to disk.
2. **Editing**:
   - Clicking "編集" (Edit) on any table row populates the form with that entry's data and switches to edit mode.
   - The form title changes to `仕訳の編集（ID: ...）` and displays a "キャンセル" (Cancel) button.
   - Clicking "更新" (Update) updates the record in-place and auto-saves.
3. **Deletion**:
   - Clicking "削除" (Delete) immediately removes the entry from memory and triggers an atomic auto-save.

### 3.4 Real-time Filtering & Sorting
Located above the ledger table, filters update the view reactively without page reloads.

- **Full-Text Match**: Partial string match against description, debit account label, or credit account label.
- **Account Filter**: Dropdown filter displaying entries where either debit or credit matches the selected account.
- **Date Range**: Inclusive start date and end date filters (`YYYY-MM-DD`).
- **Sort Toggle**:
  - `Date (Newest First)`: Date descending → Created timestamp descending.
  - `Date (Oldest First)`: Date ascending → Created timestamp ascending.
- **Real-Time Summary**: Displays active filtered count and **real-time total yen amount sum (¥)**.

---

## 4. Chart of Accounts

Includes 15 built-in accounts tailored for Japanese sole proprietors filing Blue Return forms:

| Account Label (Japanese) | Code Identifier (`AccountCode`) | Classification (`AccountNature`) | Financial Role |
|---|---|---|---|
| **現金** (Cash) | `cash` | Asset | Cash on hand |
| **普通預金** (Bank) | `bank` | Asset | Ordinary bank deposits |
| **売掛金** (Accounts Receivable) | `accounts_receivable` | Asset | Uncollected trade receivables |
| **買掛金** (Accounts Payable) | `accounts_payable` | Liability | Unpaid trade payables |
| **元入金** (Capital) | `capital` | Equity | Beginning owner equity / capital |
| **事業主貸** (Drawings) | `drawings` | Asset / Equity reduction | Business funds transferred to owner's personal use |
| **事業主借** (Owner Contribution) | `owner_contribution` | Liability / Equity addition | Personal funds injected into business |
| **売上高** (Sales) | `sales` | Income | Operating gross sales |
| **仕入高** (Cost of Sales) | `cost_of_sales` | Expense | Cost of goods purchased |
| **旅費交通費** (Travel) | `travel` | Expense | Transportation and business travel expenses |
| **通信費** (Communication) | `communication` | Expense | Phone, postage, internet subscriptions |
| **消耗品費** (Supplies) | `supplies` | Expense | Equipment under 100,000 yen, consumables, stationery |
| **地代家賃** (Rent) | `rent` | Expense | Office and facility lease payments |
| **水道光熱費** (Utilities) | `utilities` | Expense | Electricity, water, and gas expenses |
| **広告宣伝費** (Advertising) | `advertising` | Expense | Advertisements, flyers, web promotion |
| **接待交際費** (Entertainment) | `entertainment` | Expense | Business dining, gifts, ceremonial funds |
| **損害保険料** (Insurance) | `insurance` | Expense | Fire and liability insurance on business assets |
| **修繕費** (Repair) | `repair` | Expense | Repairs and maintenance of equipment |
| **福利厚生費** (Welfare) | `welfare` | Expense | Staff health checks, refreshments |
| **給料賃金** (Salaries) | `salaries` | Expense | Wages paid to staff / employees |
| **外注工賃** (Subcontracting) | `subcontracting` | Expense | Payments to external contractors and freelancers |
| **利子割引料** (Interest) | `interest` | Expense | Interest paid on business loans |
| **租税公課** (Taxes and Dues) | `taxes_and_dues` | Expense | Business enterprise tax, stamp duty, property tax |
| **荷造運賃** (Packing & Shipping) | `packing_shipping` | Expense | Freight and packaging supplies |
| **雑費** (Miscellaneous) | `miscellaneous` | Expense | Minor operating expenses not categorized elsewhere |
| **Custom Accounts** | `Custom(String)` | Configurable | User-defined custom accounts added through the Closing & Settings UI |

---

## 5. Legacy Excel CSV Import Engine

### 5.1 Supported Format
Designed to import legacy 2-row header accounting CSV files generated by Excel-based Japanese tax macros:

```csv
日付,№,借　　　　方,,,貸　　　　方,,,,,集計科目コード１,集計科目コード２,集計除外摘要文１,集計除外摘要文２,摘　　要,摘要２,摘要３
日付,№,科目ｺｰﾄﾞ,勘定科目,金　　額,科目ｺｰﾄﾞ,勘定科目,金　　額,月,重複チェック,1,3,家事消費等,雑収入,摘要,摘要２,摘要３
1月5日,1,104,その他の預金,"165,000.00",1,売上（収入）,"165,000.00",1,FALSE,165000,0,0,0,賃料:(2026年1月分),,
```

### 5.2 Automated Encoding Detection
- Automatically detects and decodes **Shift_JIS (CP932)** (Excel default on Windows) as well as **UTF-8 (with or without BOM)** without character corruption.

### 5.3 Flexible Data Parsing Logic
- **Date Normalization**:
  - Standard formats: `YYYY-MM-DD`, `YYYY/MM/DD`, `YYYY.MM.DD`
  - Japanese calendar strings: `1月5日`, `01月05日`
  - Abbreviated slash strings: `1/5`, `01/05`
  - Full-width Japanese digits (e.g. `１月５日`) normalized to standard ASCII digits.
- **Amount Parsing**:
  - Handles floating-point string formats like `"165,000.00"`.
  - Strips currency signs (`¥`, `￥`), commas, quotes, and whitespace.
- **Description Merging**:
  - Combines columns 14 (摘要), 15 (摘要２), and 16 (摘要３) into a single space-separated memo.
- **Account Mapping Heuristics**:
  1. Exact match with standard Japanese account labels.
  2. Legacy code lookup (e.g., `1` → Sales, `101` → Cash, `102`–`105` → Bank, `111`–`112` → Accounts Receivable, `201`–`215` → Accounts Payable, `301`–`303` → Capital / Drawings / Owner Contribution, `401`/`501` → Cost of Sales, `502`/`510` → Supplies, `503` → Utilities, `504` → Travel, `505` → Communication, `515` → Rent, other 500s → Miscellaneous).
  3. Substring keyword heuristics ("預金", "銀行" → Bank; "家賃" → Rent; "電気", "ガス" → Utilities, etc.).
  4. Fallback handling: Unmapped debit falls back to `Miscellaneous`, credit to `Sales`, logging an informative warning.

### 5.4 Import Safety & Preview UI
- Supports native **file picker dialog (`rfd`)** and direct **text paste**.
- **Interactive Preview Table**:
  - Shows importable record count, total yen sum, parsed rows, and skipped rows.
  - Displays top 5 sample entries in a preview table.
  - Lists account mapping warnings and row-level parse errors.
- **Import Modes**:
  - Append to existing records (default).
  - Clean replacement of all existing records (via checkbox).

---

## 6. Accounting & Tax Estimation Engine (ReportPage)

All calculations are implemented as pure, side-effect-free Rust functions covered by automated unit tests.

### 6.1 Blue Return Statement (General P&L)
1. **Gross Profit**:
   $$\text{Gross Profit} = \text{Sales} - \text{Cost of Sales}$$
2. **Total Expenses**:
   Travel + Communication + Supplies + Rent + Utilities + Miscellaneous.
3. **Ordinary Income Before Special Deduction**:
   $$\text{Ordinary Income} = \text{Gross Profit} - \text{Total Expenses}$$
4. **Blue Return Special Deduction Tiers**:
   - Selectable between **None (0 JPY)**, **100,000 JPY**, or **650,000 JPY**.
   - **Income Cap Rule**: The deduction cannot exceed ordinary income (when positive) and cannot create or expand a net loss.
5. **Business Income**:
   $$\text{Business Income} = \max(\text{Ordinary Income} - \text{Applied Special Deduction}, 0)$$
6. **Balance Sheet Balances**:
   Aggregates Cash, Bank, Accounts Receivable, Accounts Payable, Capital, Drawings, and Owner Contribution.

### 6.2 Form B Income Tax Estimate
Calculates an estimated individual income tax liability based on business earnings and statutory deductions.

1. **Total Income**: Currently equivalent to net business income.
2. **Basic Deduction (令和2年 / 2020 onward rules)**:
   - Total income $\le$ 24,000,000 JPY: **480,000 JPY**
   - 24,000,001 – 24,500,000 JPY: **320,000 JPY**
   - 24,500,001 – 25,000,000 JPY: **160,000 JPY**
   - Over 25,000,000 JPY: **0 JPY**
3. **Taxable Income**:
   $$\text{Taxable Income} = \max(\text{Total Income} - \text{Total Deductions}, 0)$$
   *Note: Truncated to the nearest 1,000 JPY in compliance with Japanese General National Tax rules.*
4. **Income Tax Brackets (7-tier statutory tax rate table)**:

| Taxable Income Range | Tax Rate | Bracket Deduction |
|---|:---:|:---:|
| 1,000 – 1,949,000 JPY | 5% | 0 JPY |
| 1,950,000 – 3,299,000 JPY | 10% | 97,500 JPY |
| 3,300,000 – 6,949,000 JPY | 20% | 427,500 JPY |
| 6,950,000 – 8,999,000 JPY | 23% | 636,000 JPY |
| 9,000,000 – 17,999,000 JPY | 33% | 1,536,000 JPY |
| 18,000,000 – 39,999,000 JPY | 40% | 2,796,000 JPY |
| 40,000,000 JPY and above | 45% | 4,796,000 JPY |

   *Note: Truncated to the nearest 100 JPY in compliance with national tax rules.*
5. **Reconstruction Surtax (復興特別所得税)**:
   $$\text{Reconstruction Surtax} = \lfloor \text{Income Tax} \times 2.1\% \rfloor \quad (\text{Truncated to 1 JPY})$$
6. **Total Income Tax Liability**:
   $$\text{Total Tax} = \text{Income Tax} + \text{Reconstruction Surtax}$$

---

## 7. PDF Report Generation

### 7.1 Exported Documents
1. **Blue Return P&L Summary PDF**:
   - Header, fiscal year, taxpayer identity (name, business name, address).
   - Sales, cost of sales, gross profit.
   - Itemized expense breakdown and total expenses.
   - Ordinary income, special deduction tier/amount, and net business income.
2. **Form B Summary PDF**:
   - Header, fiscal year, taxpayer profile.
   - Business income, total income, basic deduction.
   - Taxable income, base income tax, reconstruction surtax, and total tax amount.

### 7.2 Generation Engine & Fonts
- **Single-Binary Generation**: Uses the `printpdf` crate to generate standards-compliant PDF documents without C library dependencies.
- **Japanese Font Resolution Pipeline**:
  1. Path specified in `TAX_RETURN_FONT` environment variable.
  2. Project bundled fonts (`assets/fonts/NotoSansJP-Regular.otf`).
  3. OS system font search (IPA Gothic, IPAex Gothic, Hiragino Kaku Gothic, MS Gothic, Meiryo).
  4. ASCII fallback renderer if no CJK font is discovered.
- **Externalized Coordinate Maps**:
  - `templates/blue_return_fields.json` and `templates/form_b_fields.json`.
  - Millimeter-based coordinates allow tweaking label positions without recompilation.
- **Quick Folder Launch**:
  - Outputs to `{OS App Data}/TaxReturnApplication/exports/{year}_*.pdf`.
  - "保存先フォルダを開く" (Open Export Folder) launches Windows Explorer, macOS Finder, or Linux file manager (`xdg-open`).

---

## 8. Closing & Settings (ClosingPage)

### 8.1 Chart of Accounts & Sub-Accounts Management
- **Custom Account Register**:
  - Allows adding custom accounts with arbitrary labels (e.g. Cloud Subscriptions, Advertising, Subcontracting) and classifications (Asset, Liability, Equity, Income, Expense).
  - Validation guards prevent deletion of standard accounts or accounts actively referenced in existing journal entries.
- **Sub-Account Management**:
  - Add and delete sub-accounts (such as bank branches, specific credit cards, or key clients) tied to any account.
  - Automatically available as predictive datalist options in the journal entry form.

### 8.2 Home-Office Expense Allocation (家事按分)
- **Business Use Ratio Configuration**:
  - Configure business percentage (0–100%) on mixed-use expense accounts (rent, internet, mobile phone, electricity).
  - Supports scoping rules to specific sub-accounts.
- **Real-Time Calculation Preview**:
  - Instantly computes annual expense sum, business deduction amount, and personal share to be transferred.
- **One-Click Batch Journal Entry Generation**:
  - Clicking "家事按分仕訳を一括生成・反映" automatically generates December 31 entries:
    `Debit: Drawings (事業主貸) / Credit: Target Expense Account (with sub-account)`.
  - Re-running the allocation cleanly updates existing allocation entries without duplication.

### 8.3 Fiscal Year Rollover (年度繰越処理)
- **Balance Sheet (B/S) Carry-Forward**:
  - Carries forward ending balances of Assets (Cash, Bank, Accounts Receivable) and Liabilities (Accounts Payable) to next year's opening balances.
- **Automated Beginning Capital (元入金) Reconciliation**:
  $$\text{Next Beginning Capital} = \text{Previous Capital} + \text{Net Ordinary Income} + \text{Owner Contribution} - \text{Drawings}$$
  Reconciles all drawings, owner contributions, and net income into beginning owner equity, resetting drawings/contributions to zero for the new fiscal year.
- **Automatic Opening Journal Entry Generation**:
  - Generates January 1 opening entries (`Debit: Asset / Credit: Capital`, `Debit: Capital / Credit: Liability`).
- **Complete Master Data Inheritance**:
  - Preserves custom accounts, sub-accounts, allocation rules, taxpayer info, and blue return deduction selections for the newly created fiscal year.

---

## 9. Data Storage & Persistence

### 9.1 OS Path Conventions
Paths are resolved via the `directories` crate:

- **Windows**: `C:\Users\{Username}\AppData\Roaming\TaxReturnApplication\TaxReturnApplication\data\ledgers\{year}.json`
- **macOS**: `~/Library/Application Support/TaxReturnApplication/ledgers/{year}.json`
- **Linux**: `~/.local/share/TaxReturnApplication/ledgers/{year}.json`

### 9.2 Document Schema (Version 3)

```json
{
  "version": 3,
  "fiscal_year": 2026,
  "taxpayer": {
    "name": "Taro Yamada",
    "business_name": "Yamada Consulting",
    "address": "1-1-1 Chiyoda, Chiyoda-ku, Tokyo",
    "phone": "03-0000-0000"
  },
  "special_deduction": "yen650k",
  "accounts": [
    {
      "code": "bank",
      "name": "普通預金",
      "nature": "Asset",
      "is_custom": false,
      "sub_accounts": ["三井住友銀行", "みずほ銀行"]
    }
  ],
  "allocation_rules": [
    {
      "account": "rent",
      "business_ratio_percent": 50,
      "sub_account": null
    }
  ],
  "entries": [
    {
      "id": "e7b0c3f5-197e-49b8-a15d-85324b172a39",
      "date": "2026-01-05",
      "debit": "bank",
      "debit_sub": "三井住友銀行",
      "credit": "sales",
      "credit_sub": null,
      "amount": 165000,
      "description": "Monthly rent revenue",
      "created_at": "2026-01-05T10:00:00+09:00"
    }
  ]
}
```

### 9.3 Atomic Writes
To prevent file corruption from application crashes or power failures, data is first serialized into a temporary file (`{year}.json.tmp`) and then atomically renamed over the target file using OS-level atomic filesystem operations.

