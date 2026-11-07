# 確定申告ソフトウェア 技術ドキュメント (日本語)

本ディレクトリは、Rust / Dioxus で構築されたカスタム確定申告デスクトップアプリケーション（`TaxReturnApplication-Dioxus`）の仕様・アーキテクチャ・開発ロードマップに関するドキュメント一式を格納しています。

---

## 📚 ドキュメント構成

| ファイル | タイトル | 概要 |
|---|---|---|
| [**specifications.md**](./specifications.md) | **現行機能仕様書** | 現在（v0.1.0）実装されている全機能の網羅的仕様。仕訳帳の入力・編集・削除、リアルタイム検索・絞り込み・ソート、旧Excel 2行ヘッダーCSVインポート、青色申告決算書・申告書B税額計算ロジック、PDF出力、データ永続化仕様。 |
| [**architecture.md**](./architecture.md) | **アーキテクチャ設計書** | システム全体構造、モジュール分離、データ構造定義（`JournalEntry`, `AccountCode`, `LedgerDocument`, `TaxReportBundle` 等）、Dioxus UI状態管理、および30件の単体テストスイート構成。 |
| [**roadmap_and_commercial_gap.md**](./roadmap_and_commercial_gap.md) | **市販ソフト比較・ギャップ分析・拡充ロードマップ** | 市販の税務ソフト（freee、マネーフォワード、やよいの青色申告、国税庁確定申告書等作成コーナー）との徹底比較、現状の強みと不足機能の分析、および市販ソフトに匹敵・凌駕するための5段階の機能拡充ロードマップ。 |

---

## 🚀 クイックリンク

- **リポジトリルート README**: [README.md](../../README.md)
- **英語ドキュメント (English Documentation)**: [doc/en/README.md](../en/README.md)
- **全体インデックス**: [doc/README.md](../README.md)

