//! 货币代码（ISO 4217 三字母）

use std::fmt;

use serde::{Deserialize, Serialize};

/// 货币代码（ISO 4217 三字母）
///
/// 内部使用 `[u8; 3]` 存储，避免堆分配。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Currency(pub [u8; 3]);

impl Default for Currency {
    /// 默认 USD
    #[inline]
    fn default() -> Self {
        Self::USD
    }
}

impl Currency {
    /// 美元
    pub const USD: Currency = Currency(*b"USD");
    /// 欧元
    pub const EUR: Currency = Currency(*b"EUR");
    /// 人民币
    pub const CNY: Currency = Currency(*b"CNY");
    /// 泰达币（视为 USD 等价）
    pub const USDT: Currency = Currency(*b"UST");
    /// 比特币
    pub const BTC: Currency = Currency(*b"BTC");
    /// 以太坊
    pub const ETH: Currency = Currency(*b"ETH");

    /// 创建货币代码（截断或填充至 3 字节）
    pub fn new(code: &str) -> Self {
        let bytes = code.as_bytes();
        let mut arr = [0u8; 3];
        let n = 3.min(bytes.len());
        arr[..n].copy_from_slice(&bytes[..n]);
        Currency(arr)
    }

    /// 获取字符串视图
    #[inline]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("???")
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for Currency {
    fn from(s: &str) -> Self {
        Currency::new(s)
    }
}

impl From<Currency> for String {
    fn from(c: Currency) -> String {
        c.as_str().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(Currency::USD.as_str(), "USD");
        assert_eq!(Currency::USDT.as_str(), "UST");
        assert_eq!(Currency::BTC.as_str(), "BTC");
        assert_eq!(Currency::ETH.as_str(), "ETH");
    }

    #[test]
    fn test_new_truncates_or_pads() {
        assert_eq!(Currency::new("USDT").as_str(), "USD");
        assert_eq!(Currency::new("U").as_str(), "U\0\0");
        assert_eq!(Currency::new("EUR").as_str(), "EUR");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Currency::USD), "USD");
    }

    #[test]
    fn test_default() {
        assert_eq!(Currency::default(), Currency::USD);
    }

    #[test]
    fn test_from_str() {
        let c: Currency = "BTC".into();
        assert_eq!(c, Currency::BTC);
    }

    #[test]
    fn test_into_string() {
        let s: String = Currency::USD.into();
        assert_eq!(s, "USD");
    }
}
