//! 交易角色

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 交易角色：决定适用 Maker 还是 Taker 费率
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum TradeRole {
    /// 挂单方（提供流动性，费率较低）
    Maker,
    /// 吃单方（消耗流动性，费率较高）
    Taker,
}

impl TradeRole {
    /// 角色名称
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Maker => "maker",
            Self::Taker => "taker",
        }
    }
}

impl std::fmt::Display for TradeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_as_str() {
        assert_eq!(TradeRole::Maker.as_str(), "maker");
        assert_eq!(TradeRole::Taker.as_str(), "taker");
    }

    #[test]
    fn test_role_display() {
        assert_eq!(format!("{}", TradeRole::Maker), "maker");
        assert_eq!(format!("{}", TradeRole::Taker), "taker");
    }

    #[test]
    fn test_role_equality() {
        assert_eq!(TradeRole::Maker, TradeRole::Maker);
        assert_ne!(TradeRole::Maker, TradeRole::Taker);
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// TradeRole 在 HashSet 中可用
    #[test]
    fn test_role_in_hashset() {
        use std::collections::HashSet;
        let mut s = HashSet::new();
        s.insert(TradeRole::Maker);
        s.insert(TradeRole::Taker);
        s.insert(TradeRole::Maker);
        assert_eq!(s.len(), 2);
    }

    /// 序列化往返
    #[test]
    fn test_role_serde_roundtrip() {
        let r = TradeRole::Maker;
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("maker"));
        let de: TradeRole = serde_json::from_str(&json).unwrap();
        assert_eq!(de, TradeRole::Maker);
    }

    /// Copy 语义
    #[test]
    fn test_role_is_copy() {
        let a = TradeRole::Taker;
        let b = a; // Copy
        // a 仍可用
        assert_eq!(a, b);
    }
}
