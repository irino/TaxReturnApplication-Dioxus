//! Rule-based automated bookkeeping engine (ルールベース自動仕訳).
//!
//! Automatically classifies bank and credit card statement transactions
//! into Debit / Credit accounts based on description keywords and patterns.

use crate::core::journal::AccountCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// マッチング方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuleMatchType {
    /// 部分一致（キーワードを含む）
    #[default]
    Contains,
    /// 前方一致（キーワードから始まる）
    StartsWith,
    /// 完全一致
    Exact,
}

impl RuleMatchType {
    pub fn label_ja(self) -> &'static str {
        match self {
            Self::Contains => "部分一致",
            Self::StartsWith => "前方一致",
            Self::Exact => "完全一致",
        }
    }

    pub fn matches(&self, text: &str, pattern: &str) -> bool {
        let t = text.trim();
        let p = pattern.trim();
        if p.is_empty() {
            return false;
        }
        match self {
            Self::Contains => t.contains(p) || t.to_lowercase().contains(&p.to_lowercase()),
            Self::StartsWith => {
                t.starts_with(p) || t.to_lowercase().starts_with(&p.to_lowercase())
            }
            Self::Exact => t.eq_ignore_ascii_case(p),
        }
    }
}

/// 自動仕訳ルール
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoBookingRule {
    pub id: String,
    /// ルール名（例: "Amazon 消耗品費"）
    pub name: String,
    /// 判定キーワード（例: "Amazon", "セブン-イレブン"）
    pub pattern: String,
    /// マッチ方式
    #[serde(default)]
    pub match_type: RuleMatchType,
    /// 割り当てる科目（費用科目または収益科目）
    pub target_account: AccountCode,
    /// 補助科目（任意）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_sub_account: Option<String>,
    /// ルールの有効フラグ
    #[serde(default = "default_true")]
    pub is_active: bool,
    /// 優先度（大きい数値ほど優先）
    #[serde(default)]
    pub priority: i32,
}

fn default_true() -> bool {
    true
}

impl AutoBookingRule {
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        target_account: AccountCode,
        target_sub_account: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            pattern: pattern.into(),
            match_type: RuleMatchType::Contains,
            target_account,
            target_sub_account,
            is_active: true,
            priority: 0,
        }
    }

    pub fn matches(&self, description: &str) -> bool {
        if !self.is_active {
            return false;
        }
        self.match_type.matches(description, &self.pattern)
    }
}

/// 摘要文に対して最も優先度の高い合致ルールを検索
pub fn match_auto_rule<'a>(
    description: &str,
    rules: &'a [AutoBookingRule],
) -> Option<&'a AutoBookingRule> {
    let mut candidates: Vec<&'a AutoBookingRule> =
        rules.iter().filter(|r| r.matches(description)).collect();

    // 優先度降順（priority が大きい順）、同じならパターンの文字数が長い順（より具体的なマッチを優先）
    candidates.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| b.pattern.len().cmp(&a.pattern.len()))
    });

    candidates.into_iter().next()
}

/// 標準で提供される一般的な自動仕訳ルールセット
pub fn default_auto_rules() -> Vec<AutoBookingRule> {
    vec![
        // 消耗品費
        AutoBookingRule::new("Amazon 消耗品費", "Amazon", AccountCode::Supplies, None),
        AutoBookingRule::new("アマゾン 消耗品費", "アマゾン", AccountCode::Supplies, None),
        AutoBookingRule::new("ヨドバシカメラ", "ヨドバシ", AccountCode::Supplies, None),
        AutoBookingRule::new("ビックカメラ", "ビックカメラ", AccountCode::Supplies, None),
        AutoBookingRule::new("アスクル", "アスクル", AccountCode::Supplies, None),
        AutoBookingRule::new("モノタロウ", "モノタロウ", AccountCode::Supplies, None),

        // 旅費交通費
        AutoBookingRule::new("Suica / PASMO", "Suica", AccountCode::Travel, None),
        AutoBookingRule::new("PASMO", "PASMO", AccountCode::Travel, None),
        AutoBookingRule::new("JR東日本 / 西日本", "JR", AccountCode::Travel, None),
        AutoBookingRule::new("地下鉄・メトロ", "メトロ", AccountCode::Travel, None),
        AutoBookingRule::new("タクシー", "タクシー", AccountCode::Travel, None),
        AutoBookingRule::new("タイムズ / 駐車場", "タイムズ", AccountCode::Travel, None),

        // 通信費
        AutoBookingRule::new("NTT 通信費", "NTT", AccountCode::Communication, None),
        AutoBookingRule::new("KDDI 通信費", "KDDI", AccountCode::Communication, None),
        AutoBookingRule::new("ソフトバンク 通信費", "ソフトバンク", AccountCode::Communication, None),
        AutoBookingRule::new("SoftBank", "SoftBank", AccountCode::Communication, None),
        AutoBookingRule::new("ドコモ", "ドコモ", AccountCode::Communication, None),
        AutoBookingRule::new("Google Cloud / サービス", "Google", AccountCode::Communication, None),
        AutoBookingRule::new("AWS クラウドインフラ", "AWS", AccountCode::Communication, None),
        AutoBookingRule::new("GitHub", "GitHub", AccountCode::Communication, None),
        AutoBookingRule::new("OpenAI / ChatGPT", "OpenAI", AccountCode::Communication, None),
        AutoBookingRule::new("郵便局 / 切手・送料", "郵便局", AccountCode::Communication, None),

        // 水道光熱費
        AutoBookingRule::new("東京電力", "東京電力", AccountCode::Utilities, None),
        AutoBookingRule::new("関西電力", "関西電力", AccountCode::Utilities, None),
        AutoBookingRule::new("東京ガス", "東京ガス", AccountCode::Utilities, None),
        AutoBookingRule::new("水道局", "水道局", AccountCode::Utilities, None),

        // 雑費
        AutoBookingRule::new("セブン-イレブン", "セブン", AccountCode::Miscellaneous, None),
        AutoBookingRule::new("ローソン", "ローソン", AccountCode::Miscellaneous, None),
        AutoBookingRule::new("ファミリーマート", "ファミリーマート", AccountCode::Miscellaneous, None),
        AutoBookingRule::new("銀行振込手数料", "振込手数料", AccountCode::Miscellaneous, None),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_matching() {
        let rules = default_auto_rules();

        let r1 = match_auto_rule("AMAZON.CO.JP 利用代金", &rules);
        assert!(r1.is_some());
        assert_eq!(r1.unwrap().target_account, AccountCode::Supplies);

        let r2 = match_auto_rule("モバイルSuica チャージ", &rules);
        assert!(r2.is_some());
        assert_eq!(r2.unwrap().target_account, AccountCode::Travel);

        let r3 = match_auto_rule("セブン-イレブン渋谷店", &rules);
        assert!(r3.is_some());
        assert_eq!(r3.unwrap().target_account, AccountCode::Miscellaneous);

        let r4 = match_auto_rule("未知の支払先 12345", &rules);
        assert!(r4.is_none());
    }

    #[test]
    fn test_priority_and_length_precedence() {
        let mut rules = vec![
            AutoBookingRule::new("一般IT", "IT", AccountCode::Miscellaneous, None),
            AutoBookingRule::new("Google Workspace", "Google", AccountCode::Communication, None),
        ];

        let matched = match_auto_rule("Google IT サービス", &rules).unwrap();
        // Google has 6 chars vs IT has 2 chars
        assert_eq!(matched.target_account, AccountCode::Communication);

        // Explicit priority override
        rules[0].priority = 10;
        let matched2 = match_auto_rule("Google IT サービス", &rules).unwrap();
        assert_eq!(matched2.target_account, AccountCode::Miscellaneous);
    }
}
