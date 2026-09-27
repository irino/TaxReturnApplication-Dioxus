# AGENTS.md: Project Specification & Development Guide

## 1. Project Overview & Objectives
- **Project Name:** Custom Tax Return Software (Desktop Application)
- **Primary Goal:** Replace a legacy Excel-macro-based tax tool with a secure, spec-open, custom-built application.
- **Key Requirements:** 
  1. Input and manage financial ledgers (double-entry bookkeeping style but tailored for ease of use).
  2. Generate official Japanese Tax Return PDF forms matching National Tax Agency (国税庁) layouts.
- **Portability:** Must compile into a single executable binary (Single Binary Delivery) across multiple desktop platforms (Windows and macOS).

---

## 2. Technology Stack & Strategy

### Frontend & App Shell: Dioxus (Desktop)
- **Rationale:** 100% Pure Rust stack. Eliminates IPC overhead and type duplication between frontend and backend. Enables seamless single-binary distribution.
- **UI Design Strategy (Crucial):** Do NOT implement a complex Excel-like grid view from scratch. Instead, build a **"Modern Input Form + Scorable/Filterable Ledger List Table"** architecture. This ensures a fast, native-feeling UI without getting bogged down in low-level web-grid emulation in Rust.

### PDF Generation: Target Sheet Engine
- **Strategy:** Embed the official blank PDF templates from the National Tax Agency into the binary via Rust's `include_bytes!` macro. 
- **Implementation:** Use the `lopdf` crate to programmatically overlay text, numbers, and calculation outcomes onto precise X/Y coordinates of the embedded PDF. Keep coordinates in an externalizable format (e.g., JSON or struct maps) to easily accommodate annual tax form updates.

### Data Storage & Portability
- **Strategy:** Serialize ledgers directly using `serde` (JSON/YAML) on a "one file per fiscal year" basis, or utilize a local SQLite database (`rusqlite`).
- **OS Integration:** Use the `directories` crate to securely pinpoint standard local app data directories (`AppData`, `Application Support`) depending on the OS.

---

## 3. Architecture & Module Design
Maintain a strict separation between UI, logic, and storage. The codebase should follow this directory schema:

- `src/main.rs`: Application entry point and window orchestration.
- `src/ui/`: Dioxus UI components.
  - `layout.rs`: Shared shell layout, sidebar navigation.
  - `ledger.rs`: Input forms and journal entries list.
  - `report.rs`: Tax document configuration and PDF generation preview.
- `src/core/`: Pure Rust tax and accounting logic. **Absolutely decoupled from UI.**
  - `journal.rs`: Journal structures, accounts chart, and bookkeeping validation.
  - `tax_calc.rs`: Aggregation logic for the Blue Return Financial Statement (青色申告決算書) and Tax Return Form B (確定申告書B).
- `src/storage/`: Data persistence layers (serialization, deserialization, and local system paths).

---

## 4. Initial Implementation Milestones

### Phase 1: Environment & State Foundation
1. Bootstrap project using `dx new --platform desktop`.
2. Define core data models (`JournalEntry`, `AccountCode`) inside `src/core/journal.rs`.
3. Set up the primary global state container using Dioxus `use_signal`.

### Phase 2: Ledger UI Implementation
1. Build a streamlined input form component for inserting transactions (Date, Debit/Credit Account, Amount, Description).
2. Wire up the form to save records into the global signal array and render them immediately on a reactive list view.

### Phase 3: Storage Integration
1. Implement local file persistence inside `src/storage/` via `serde_json` and `directories`.
2. Ensure the app automatically loads the previous sessions' records upon startup.

---

## 5. Agent Rules & Constraints
- **Pure Rust:** Do not inject Javascript/Typescript into the UI layer. All UI components must utilize Dioxus RSX syntax.
- **Robust Logic:** Write unit tests (`#[test]`) for all accounting formulas and aggregations inside `src/core/`. The calculation engine must be highly reliable.
- **No Side-Effects in Logic:** Keep functions in `src/core/` pure (`fn calculate(data: Input) -> Output`) so they can easily be tested without mocking the UI state.
