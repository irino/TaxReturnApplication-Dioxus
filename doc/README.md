# Tax Return Application Documentation / 確定申告ソフトウェア 技術ドキュメント

ようこそ！ 本ディレクトリ（`/doc`）には、Pure Rust / Dioxus で開発された確定申告デスクトップアプリケーション（`TaxReturnApplication-Dioxus`）の現行仕様、システムアーキテクチャ、および市販ソフトウェアに匹敵するための拡充ロードマップに関する詳細ドキュメントが格納されています。日本語および英語のバイリンガル構成となっています。

Welcome! This directory (`/doc`) houses the complete specifications, internal software architecture, and commercial expansion roadmap for the Rust / Dioxus tax return desktop application (`TaxReturnApplication-Dioxus`), fully provided in both Japanese and English.

---

## 📖 言語を選択してください / Select Language

### 🇯🇵 [日本語ドキュメント (Japanese)](./ja/README.md)
- [**現行仕様書 (specifications.md)**](./ja/specifications.md): 現在（v0.1.0）実装されている全機能の網羅的仕様（仕訳管理、CSVインポート、税額計算、PDF出力、データ永続化）。
- [**アーキテクチャ設計書 (architecture.md)**](./ja/architecture.md): 内部モジュール設計、データ構造、UIステート管理、30件の単体テスト構成。
- [**市販ソフト比較・ギャップ分析・拡充ロードマップ (roadmap_and_commercial_gap.md)**](./ja/roadmap_and_commercial_gap.md): freee・マネーフォワード・弥生等との詳細比較、不足機能の分析、5段階の開発ロードマップ。

### 🇺🇸 [English Documentation](./en/README.md)
- [**Current Specifications (specifications.md)**](./en/specifications.md): Complete specifications of currently implemented features (journal entry lifecycle, legacy CSV import, tax calculations, PDF engine, persistence).
- [**Architecture Design (architecture.md)**](./en/architecture.md): Internal layered architecture, Rust data structures, Dioxus state management, and 30-case test suite.
- [**Commercial Gap Analysis & Roadmap (roadmap_and_commercial_gap.md)**](./en/roadmap_and_commercial_gap.md): Comparison with commercial software (freee, Money Forward, Yayoi), functional gap breakdown, and a 5-phase engineering roadmap.

---

## 📂 ドキュメント構造 / Directory Structure

```
doc/
├── README.md                           # 全体インデックス（本ファイル） / Top-level Index
├── ja/                                 # 日本語ドキュメント / Japanese Documentation
│   ├── README.md                       # 日本語目次
│   ├── specifications.md               # 現行機能仕様書
│   ├── architecture.md                 # アーキテクチャ設計書
│   └── roadmap_and_commercial_gap.md   # 市販ソフト比較・ギャップ分析・拡充ロードマップ
└── en/                                 # 英語ドキュメント / English Documentation
    ├── README.md                       # English Index
    ├── specifications.md               # Current Feature Specifications
    ├── architecture.md                 # Architecture Design
    └── roadmap_and_commercial_gap.md   # Commercial Gap Analysis & Expansion Roadmap
```

