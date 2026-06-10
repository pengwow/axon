//! 买卖方向

use std::fmt;

use serde::{Deserialize, Serialize};

/// 买卖方向枚举
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Side {
    /// 买入（默认方向）
    #[default]
    Buy = 0,
    /// 卖出
    Sell = 1,
}

impl Side {
    /// 返回反向方向
    #[inline]
    pub const fn opposite(self) -> Side {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }

    /// 买方返回 +1，卖方返回 -1
    #[inline]
    pub const fn sign(self) -> i8 {
        match self {
            Self::Buy => 1,
            Self::Sell => -1,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => write!(f, "BUY"),
            Self::Sell => write!(f, "SELL"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_opposite() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.opposite(), Side::Buy);
    }

    #[test]
    fn test_side_sign() {
        assert_eq!(Side::Buy.sign(), 1);
        assert_eq!(Side::Sell.sign(), -1);
    }

    #[test]
    fn test_side_display() {
        assert_eq!(format!("{}", Side::Buy), "BUY");
        assert_eq!(format!("{}", Side::Sell), "SELL");
    }

    #[test]
    fn test_side_default() {
        assert_eq!(Side::default(), Side::Buy);
    }

    #[test]
    fn test_side_json_serialization() {
        let buy_json = serde_json::to_string(&Side::Buy).unwrap();
        let sell_json = serde_json::to_string(&Side::Sell).unwrap();
        let buy_restored: Side = serde_json::from_str(&buy_json).unwrap();
        let sell_restored: Side = serde_json::from_str(&sell_json).unwrap();
        assert_eq!(buy_restored, Side::Buy);
        assert_eq!(sell_restored, Side::Sell);
    }
}
