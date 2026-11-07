# Commercial Tax Software Comparison, Gap Analysis & Expansion Roadmap

This document provides a **detailed comparison against commercial tax return software (freee, Money Forward Cloud Tax, Yayoi Blue Return, National Tax Agency NTA e-Tax portal), identifies functional gaps, and outlines a phased engineering roadmap** to elevate `TaxReturnApplication-Dioxus` into a full-featured tax and accounting platform.

---

## 1. Market Positioning & Inherent Strengths

### 1.1 Commercial Software Categories
Modern tax return software in Japan falls into three main categories:

1. **Cloud SaaS (freee, Money Forward Cloud Tax Return)**:
   - *Strengths*: Direct bank/credit card API synchronization, mobile apps, automated transaction categorization.
   - *Weaknesses*: Recurring subscription fees (10,000 to 30,000+ JPY annually), mandatory internet connectivity, privacy risks from storing sensitive financial records on external cloud servers, forced UI/workflow updates.
2. **On-Premise Desktop Suites (Yayoi Blue Return, Sorimachi, etc.)**:
   - *Strengths*: Comprehensive accounting reports, standard double-entry workflows, rapid keyboard data entry.
   - *Weaknesses*: Windows-only in most cases, paid annual version upgrades, proprietary binary database formats causing strong vendor lock-in.
3. **Government Portal (National Tax Agency "確定申告書等作成コーナー" / e-Tax)**:
   - *Strengths*: Free of charge, perfectly aligned with statutory tax revisions, direct electronic filing.
   - *Weaknesses*: Lacks day-to-day bookkeeping and general ledger features (users must maintain external books and manually enter summary figures), cumbersome UI that changes every year.

### 1.2 Inherent Strengths of TaxReturnApplication-Dioxus

| Aspect | TaxReturnApplication-Dioxus | Commercial Cloud Software |
|---|---|---|
| **Privacy & Security** | **100% Local Execution**. Zero external network requests. Financial ledgers never leave the user's local machine. | Banking credentials and sensitive transactions stored on third-party cloud infrastructure. |
| **Operating Cost** | **Completely Free Forever**. No recurring monthly or annual subscription fees. | 10,000 to 30,000+ JPY annual subscription fees indefinitely. |
| **Open Data Specification** | **Human-Readable JSON**. Full revision history via Git, transparent schema, easily manipulated via shell scripts. | Proprietary formats making data export and migration difficult. |
| **Performance & Resource Footprint** | **Pure Rust Single Binary**. Sub-second startup, tens of megabytes of RAM, no Electron or web browser bloat. | Heavy web browsers, sluggish on older hardware, unusable offline. |
| **Cross-Platform Delivery** | Compiles to native single binaries for Windows, macOS, and Linux. | Often lacks Linux support or restricts macOS capabilities. |

---

## 2. Comprehensive Gap Analysis Against Commercial Software

To bridge the gap between version 0.1.0 and commercial-grade tools, missing features are categorized below:

### 2.1 Accounting & Bookkeeping Capabilities

| Feature | Commercial Suites (freee/Yayoi) | Current App (v0.1.0) | Gap & Engineering Requirements |
|---|:---:|:---:|---|
| **Custom Chart of Accounts** | Add, edit, delete, and reorder accounts freely | Fixed 15 accounts | Support user-defined accounts with classification (B/S, P&L) and reporting categories |
| **Sub-Accounts & Tags** | Sub-accounts per bank account, credit card, or client | None (Single generic Bank and Accounts Receivable) | Enable tracking balances per entity (e.g., "Bank: SMBC", "Accounts Receivable: Client A") |
| **Compound Transactions (Split/Multi-Leg)** | Multiple debits and credits per transaction | Single debit : single credit only | Necessary for handling split payments, bank fees deducted at source, and withholding taxes |
| **General Ledger & Subsidiary Ledgers** | View, filter, and print transactions itemized per account | Account balances only (no drill-down ledger view) | Interactive ledger view per account with running balances and direct in-place editing |
| **Recurring Journal Templates / Dictionaries** | One-click entry of recurring monthly transactions | Manual form entry | Save and recall standard transactions (monthly rent, internet, recurring software subscriptions) |
| **Monthly Trend Statements & Trial Balances** | Month-by-month sales and expense trajectory | Annual totals only | Monthly P&L table to analyze revenue seasonality and cost spikes before filing |

### 2.2 Year-End Adjustments & Asset Management

| Feature | Commercial Suites (freee/Yayoi) | Current App (v0.1.0) | Gap & Engineering Requirements |
|---|:---:|:---:|---|
| **Home-Office Cost Allocation (家事按分)** | Automatic percentage split of rent, electricity, internet between business and personal | Requires manual calculation and entry | Set business-use percentages (e.g., Rent 50%, Power 30%) and generate automated year-end adjustments |
| **Fixed Assets & Depreciation Ledger (減価償却)** | Straight-line, declining-balance, lump-sum (3-year), and small asset special deduction (under 300k JPY) | None (Assets expensed immediately) | Asset master register, statutory useful life tables, automated yearly depreciation calculation |
| **Fiscal Year Roll-Over (年度繰越)** | Automatic rollover of ending B/S balances into next year's opening balances | Disjoint independent annual files | Calculate ending retained earnings, drawings, and contributions to update next year's opening Capital (元入金) |
| **Inventory & Cost of Goods Sold (棚卸)** | Opening inventory + Purchases - Ending inventory = COGS | Aggregates purchases only | Record opening and ending inventory values to compute true Cost of Goods Sold |

### 2.3 Consumption Tax & Invoice System (インボイス制度)

| Feature | Commercial Suites (freee/Yayoi) | Current App (v0.1.0) | Gap & Engineering Requirements |
|---|:---:|:---:|---|
| **Tax Categories per Entry** | 10% standard, 8% reduced, non-taxable, exempt, transitional measures | None (Gross amounts without tax codes) | Tag transactions with tax codes; support tax-inclusive vs. tax-exclusive accounting |
| **Consumption Tax Filing** | General taxation, simplified taxation (classes 1–6), small-business 20% special rule | None | Aggregate taxable sales/purchases and calculate consumption tax liabilities |
| **Qualified Invoice Issuer (T-Number)** | Track counterparty invoice registration numbers | None | Verify and record qualified invoice registration status for input tax credit calculations |

### 2.4 Comprehensive Income Tax Deductions

| Feature | Commercial Suites / NTA Portal | Current App (v0.1.0) | Gap & Engineering Requirements |
|---|:---:|:---:|---|
| **Statutory Income Deductions** | All 15 deductions (Social Insurance, Small Business Mutual Aid, Life Insurance, Earthquake, Medical, Donations, Spouse, Dependents) | Basic Deduction only | Dedicated deduction input UI, automated phase-outs, and multi-tier life insurance formulas |
| **Tax Credits (税額控除)** | Mortgage interest credit (住宅ローン控除), dividend tax credit | None | Deduction directly from calculated income tax liability on Form B |
| **Multiple Income Categories** | Real estate income, employment income, capital gains, miscellaneous income | Business income only | Support separate real estate P&L, employment income deductions, and aggregate/separate taxation |
| **Local Tax Declarations** | Furusato Nozei one-stop opt-out, resident tax collection method (by payroll or direct) | None | Support Page 2 resident tax and enterprise tax configuration |

### 2.5 External Integrations & E-Filing

| Feature | Commercial Suites / NTA Portal | Current App (v0.1.0) | Gap & Engineering Requirements |
|---|:---:|:---:|---|
| **Data Storage & Portability** | Proprietary cloud DB / Proprietary binary | Custom JSON (Split per fiscal year) | Single-file management (SQLite), interoperability with open standards such as GnuCash and Beancount |
| **Generic Bank/Card CSV Import** | Map arbitrary CSV columns and configure automated rule engines | Legacy Excel CSV only | Visual CSV column mapper and rule-based auto-categorization ("Amazon" → Supplies) |
| **Official PDF Overlay Printing** | Imprint directly onto official multi-page NTA tax forms | Custom monochrome summary layout | Exact coordinate overlay onto official blank NTA PDF forms ready for tax office submission |
| **e-Tax Data Export (XML / .xtx)** | Export data files importable into NTA e-Tax software | PDF export only | Generate NTA-compliant XML/.xtx schema files for paperless filing with My Number cards |

---

## 3. Architectural Comparison of Data Storage Formats (Proprietary JSON vs GnuCash SQLite vs Beancount)

To chart the best evolutionary path for this application's persistence layer, we analyze and compare three approaches: **GnuCash (SQLite)**, **Beancount (Plain Text Accounting)**, and the **current proprietary JSON format**.

### 3.1 Characteristic Comparison

| Dimension | Current: Proprietary JSON (Annual Split) | GnuCash Compatible (Single-File SQLite3) | Beancount (Plain Text Accounting) |
|---|---|---|---|
| **File Management** | One file per fiscal year (`{year}.json`) | **All historical years in a single file (`.gnucash`)** | Single file (or multiple `.beancount` files) |
| **Interoperability** | ✕ Closed proprietary format (no external tools) | **◎ Viewable & editable directly in GnuCash desktop** | **◎ Viewable & editable in Beancount / Fava** |
| **Large-Data Performance** | △ Parse & write entire tree slows down as entries grow | **◎ Ultra-fast queries with B-Tree indexes & SQL** | △ Parsing overhead increases beyond tens of thousands of entries |
| **Transaction Safety** | ◯ Atomic file replacement via temporary files | **◎ Full ACID transactions backed by SQLite** | △ Whole-file overwrite (no row-level atomic guarantees) |
| **Git / Text Diffs** | ◯ Standard JSON text diffs | ✕ Binary DB prevents meaningful text diffs | **◎ Gold standard (line-by-line pristine diffs)** |
| **Compound Entries** | ✕ 1 debit : 1 credit only | **◎ Native standard via `transactions` + `splits`** | **◎ Native standard via multi-line postings** |
| **Japanese Tax Metadata** | ◎ Fully optimized domain-specific schema | △ Stored via auxiliary `slots` key-value table | △ Stored via custom tags or metadata lines |

### 3.2 GnuCash SQLite Architecture Details
- **Structure**: Relational schema composed of `books` (bookkeeping root), `accounts` (hierarchical account tree), `transactions` (transaction header), `splits` (individual debit/credit line items), and `slots` (arbitrary key-value metadata extension table).
- **Strengths**:
  1. **Single-File Self-Containment**: Backing up or moving a single file (e.g. `accounting.gnucash`) preserves the entire historical record.
  2. **GnuCash Ecosystem**: Users can open the file in GnuCash for deep trial balances, multi-currency valuation, or securities portfolio management.
  3. **Standardized Compound Splits**: Multi-leg entries (fee deductions, split bank payouts) are natively supported.
- **Challenges**: Requires implementing GnuCash-specific types (16-byte hex GUID strings, rational numbers `value_num`/`value_denom`) and mapping Japanese tax metadata (Blue Return 650,000 JPY deduction flags, allocation rules) into the `slots` table.

### 3.3 Beancount (Plain Text Accounting) Architecture Details
- **Structure**: Human-readable plain text format.
  ```beancount
  2026-01-05 * "Rent Payment" "January Office Rent"
    Assets:Bank:SMBC            165000 JPY
    Income:Sales               -165000 JPY
  ```
- **Strengths**:
  1. **Ultimate Git Harmony**: Full attribution of who changed which transaction, when, and why down to the exact line in commit history.
  2. **Fava Web UI**: Users can launch Fava to inspect high-quality financial charts, balance sheets, and P&L statements.
- **Challenges**: Performing programmatic updates and deletions of individual entries by ID requires manipulating a text file's concrete syntax tree, incurring higher consistency overhead compared to relational SQLite.

### 3.4 Recommended Storage Evolution Strategy (Hybrid Single-File Approach)
To satisfy the user needs of **"managing all years in a single file"** and **"leveraging GnuCash's SQLite format"**, we adopt a **hybrid storage strategy**:

1. **Migrate Primary Storage to Single-File SQLite**:
   - Transition the application's core persistence layer from per-year JSON files to a **single SQLite file (`tax_return.sqlite` / `tax_return.gnucash`)**.
   - Adopt a schema compatible with GnuCash standard tables (`accounts`, `transactions`, `splits`), storing all years and entries in one unified file.
2. **Interoperability Layer (GnuCash & Beancount Import/Export)**:
   - **GnuCash Interoperability**: Open SQLite files created in GnuCash directly in this app to generate Japanese Blue Return and Form B PDFs.
   - **Beancount Export**: One-click export of transactions to `.beancount` plain text format (satisfying developers who want Git tracking and Fava visualization).
   - **Automated Migration from Legacy JSON**: Automatically detect existing per-year JSON files and migrate all historical records into the unified SQLite database.

---

## 4. Phased Expansion Roadmap

To systematically expand capabilities while maintaining codebase stability and test coverage, the following 6-phase roadmap is established:

```
[Phase 1: Advanced Bookkeeping & Year-End Foundations] (Completed ✅)
  ├── Customizable Chart of Accounts & Sub-Accounts (Implemented)
  ├── Home-Office Expense Allocation Engine (家事按分) (Implemented)
  ├── Fiscal Year Balance Roll-Over (B/S 繰越処理) (Implemented)
  └── Closing & Settings Page (ClosingPage) (Implemented)
       ↓
[Phase 2: Depreciation Ledger & Complete Statutory Deductions] (Next Milestone 🎯)
  ├── Fixed Asset Register & Automated Depreciation Engine (Straight-line, 3-year, Small asset)
  ├── Complete Income Tax Deductions (Social insurance, iDeCo, Life insurance, Medical, Donations)
  └── Mortgage Interest Tax Credit & Form B Expansion
       ↓
[Phase 3: Single-File SQLite & GnuCash / Beancount Interoperability] (Data Storage Modernization 📂)
  ├── Single-file SQLite persistence (Consolidate all years and masters into one .sqlite/.gnucash)
  ├── GnuCash SQLite compatible schema integration (Direct inspection & interoperability)
  ├── Full compound transaction support (transactions + splits data model)
  ├── Beancount plain text export & import (Git diffs & Fava ecosystem)
  └── Automated one-click migration from legacy annual JSON files to SQLite
       ↓
[Phase 4: Official PDF Overlays & e-Tax XML Export]
  ├── Pixel-perfect overlay on official NTA blank PDF templates (Blue Return 1–4, Form B 1–2)
  └── e-Tax XML / .xtx file export generator for direct online filing
       ↓
[Phase 5: Universal Financial CSV Import & Rule-Based Automation]
  ├── Universal Bank / Credit Card CSV column mapper
  ├── Rule-based automated bookkeeping engine ("Keyword" -> Debit/Credit/Account)
  └── Multi-leg / split transaction UI (Compound entries)
       ↓
[Phase 6: Invoice System, Consumption Tax & Business Analytics]
  ├── Consumption tax rates (10% / 8%), Invoice T-number management
  ├── Simplified taxation, general taxation, and 20% transitional tax calculations
  └── Monthly revenue trend charts and cash flow analytics
```

---

## 5. Phase-by-Phase Detailed Implementation Plans

### Phase 1: Advanced Bookkeeping & Year-End Foundations (Completed ✅)
- **Account Customization**:
  - Expanded standard Blue Return accounts and added extensible `AccountCode::Custom(String)` variant with untagged serde deserialization.
  - Added `AccountDefinition` registry managing account names, natures, and sub-account lists.
  - Sub-accounts freely configurable per account, displayed as badges, and auto-suggested in transaction input forms.
  - Permitted inter-subaccount fund transfer entries (e.g. Bank A to Bank B) with validation safety.
- **Home-Office Cost Allocation (家事按分)**:
  - Allocation rules with configurable business percentage (0–100%) and optional sub-account targeting.
  - Automated generation of December 31 entries (`Debit: Drawings / Credit: Target Account`) with clean updates on re-runs.
- **Year-End Rollover Engine**:
  - Carries forward ending B/S balances of Assets and Liabilities into next year's opening balances.
  - Automatically calculates ending owner equity ($Next\ Capital = Capital + Income + Contributions - Drawings$) and posts January 1 opening journal entries.
  - Automatically creates and navigates to the newly initialized fiscal year.
- **Closing & Settings Screen (ClosingPage)**:
  - New 3-tab UI shell dedicated to chart of accounts, home-office allocation, and fiscal rollover.

### Phase 2: Depreciation Ledger & Statutory Deductions (Next Milestone 🎯)
- **Fixed Asset Register**:
  - Model asset properties: Asset Name, Acquisition Date, Cost, Useful Life, and Depreciation Method (Straight-line, 3-year lump sum, Immediate expensing under 300,000 JPY for blue return).
  - Compute yearly depreciation expense, automatically generate year-end journal entries, and populate Page 3 of the Blue Return statement.
- **Statutory Deduction Manager**:
  - Social Insurance (National Health Insurance, National Pension).
  - Small Business Mutual Aid & iDeCo contributions.
  - Life Insurance (General, Medical Care, Nursing Care, Individual Annuity under new and old regimes).
  - Earthquake Insurance, Medical Expense Deduction, and Donation Deduction (Furusato Nozei).
  - Spouse and Dependent exemptions.

### Phase 3: Single-File SQLite & GnuCash / Beancount Interoperability (Data Storage Modernization 📂)
- **Single-File SQLite Persistence (`rusqlite`)**:
  - Consolidate all fiscal years of journal entries, chart of accounts, sub-accounts, and taxpayer info into a single SQLite database file (e.g. `tax_return.sqlite` / `tax_return.gnucash`).
  - Back up and migrate the entire accounting history simply by handling a single file.
  - SQLite's B-Tree indexing and ACID transaction engine guarantee instantaneous querying across hundreds of thousands of entries.
- **GnuCash Compatible Schema Integration**:
  - Conform to GnuCash's standard relational design (`books`, `accounts`, `transactions`, `splits`, `slots`).
  - Directly open GnuCash-managed SQLite files within this app to generate official Japanese Form B and Blue Return financial reports.
  - Support full compound entries (multiple debits and credits under one transaction header) in the core domain model.
- **Beancount Plain Text I/O**:
  - Export and import journal entries to and from `.beancount` plain text format with a single click.
  - Provide maximum portability for developers seeking line-by-line Git version control and integration with Fava web dashboards.
- **Automated Migration from Legacy Storage**:
  - Auto-detect legacy per-year JSON files (schema v1–v3) and seamlessly convert all records into the single SQLite file.

### Phase 4: Official PDF Overlays & e-Tax XML Export (Filing-Grade Quality)
- **Official NTA PDF Template Imprinting**:
  - Embed official National Tax Agency blank PDF forms via `include_bytes!`.
  - Draw filled text and numbers onto the exact coordinates of official color templates:
    - Blue Return Financial Statement: Page 1 (P&L), Page 2 (Monthly revenue & payroll), Page 3 (Depreciation & rent details), Page 4 (Balance sheet).
    - Form B: Page 1 (Income, deductions, tax calculation), Page 2 (Breakdown of income, family details, local tax options).
  - Produce filings ready for immediate print and mail submission to tax offices.
- **e-Tax XML File Generation**:
  - Format accounting and tax data into official NTA e-Tax XML structures, enabling users to upload their return directly to the e-Tax Web portal using a My Number card reader.

### Phase 5: Universal Financial CSV Import & Automation (Substantial UX Boost)
- **Universal CSV Importer**:
  - Provide an interactive UI to map arbitrary CSV columns from any Japanese bank or credit card company (Date, Description, Withdrawal Amount, Deposit Amount).
- **Automated Bookkeeping Rule Engine**:
  - Allow users to establish rules such as: *If description contains "東京電力", set Debit = Utilities, Credit = Bank*.
  - Bulk-categorize transaction records, slashing daily bookkeeping time by 90%.

### Phase 6: Invoice System, Consumption Tax Filing & Business Analytics (Reaching Commercial Parity)
- **Consumption Tax Engine**:
  - Track 10% standard, 8% reduced, and tax-exempt transactions for qualified invoice issuers.
  - Compute tax returns under both Simplified Taxation (みなし仕入率) and the 20% Special Rule (2割特例) for small businesses.
  - Generate official Consumption Tax and Local Consumption Tax return PDFs.
- **Visual Analytics Dashboard**:
  - Interactive SVG/Canvas charts for monthly revenue trends, expense breakdown ratios, and year-over-year comparisons.

---

## 6. Summary & Recommended Next Actions

With the completion of Phase 1, **a robust double-entry bookkeeping and fiscal rollover foundation has been established** (customizable chart of accounts, sub-account badges, home-office expense allocation, and balance sheet rollover).

The recommended next path is to methodically implement **Phase 2: Fixed Assets / Depreciation & Complete Statutory Income Deductions** to satisfy formal tax return filing requirements, followed directly by **Phase 3: Single-File SQLite Modernization (GnuCash & Beancount Interoperability)** to unify multi-year storage and unlock broad open-source ecosystem compatibility.
