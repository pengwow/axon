//! 交易品种代码
//!
//! 例如 `"BTC-USDT"`、`"AAPL"`、`"600519.SH"`。

use serde::{Deserialize, Serialize};

/// 交易品种代码（newtype 包装 `String`）
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Symbol(String);

impl Symbol {
    /// 取引种字符串引用
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 获取字符串长度
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 是否为空
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for Symbol {
    #[inline]
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for Symbol {
    #[inline]
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_from_str() {
        let s = Symbol::from("BTC-USDT");
        assert_eq!(s.as_str(), "BTC-USDT");
    }

    #[test]
    fn test_symbol_display() {
        let s = Symbol::from("ETH-USDT");
        assert_eq!(format!("{s}"), "ETH-USDT");
    }

    #[test]
    fn test_symbol_equality() {
        let a = Symbol::from("BTC-USDT");
        let b = Symbol::from("BTC-USDT");
        let c = Symbol::from("ETH-USDT");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_symbol_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Symbol::from("BTC-USDT"));
        set.insert(Symbol::from("BTC-USDT"));
        set.insert(Symbol::from("ETH-USDT"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_symbol_from_string() {
        let owned = String::from("AAPL");
        let s = Symbol::from(owned);
        assert_eq!(s.as_str(), "AAPL");
    }

    #[test]
    fn test_symbol_is_empty() {
        let s = Symbol::from("");
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }
}
