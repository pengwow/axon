//! 订单有效期（Time In Force）

use std::fmt;

use serde::{Deserialize, Serialize};

/// 订单有效期
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good Till Cancelled - 一直有效直到成交或取消
    #[default]
    GTC,
    /// Immediate Or Cancel - 立即成交可成交部分，剩余取消
    IOC,
    /// Fill Or Kill - 必须全部成交，否则全部取消
    FOK,
    /// Good For Day - 当日有效
    GFD,
    /// Fill And Kill - FOK 别名
    FAK,
}

impl fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GTC => write!(f, "GTC"),
            Self::IOC => write!(f, "IOC"),
            Self::FOK => write!(f, "FOK"),
            Self::GFD => write!(f, "GFD"),
            Self::FAK => write!(f, "FAK"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tif_default_is_gtc() {
        assert_eq!(TimeInForce::default(), TimeInForce::GTC);
    }

    #[test]
    fn test_tif_display() {
        assert_eq!(format!("{}", TimeInForce::GTC), "GTC");
        assert_eq!(format!("{}", TimeInForce::IOC), "IOC");
        assert_eq!(format!("{}", TimeInForce::FOK), "FOK");
        assert_eq!(format!("{}", TimeInForce::GFD), "GFD");
        assert_eq!(format!("{}", TimeInForce::FAK), "FAK");
    }

    #[test]
    fn test_tif_equality() {
        assert_eq!(TimeInForce::GTC, TimeInForce::GTC);
        assert_ne!(TimeInForce::GTC, TimeInForce::IOC);
    }
}
