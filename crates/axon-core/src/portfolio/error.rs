//! 投资组合错误

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::currency::Currency;
use crate::types::Symbol;

/// 投资组合错误
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortfolioError {
    /// 持仓不存在
    #[error("持仓不存在：{symbol}")]
    PositionNotFound {
        /// 标的代码
        symbol: Symbol,
    },

    /// 现金不足
    #[error("现金不足：{currency} 需要 {required}，可用 {available}")]
    InsufficientCash {
        /// 货币
        currency: Currency,
        /// 需要的金额
        required: i64,
        /// 可用金额
        available: i64,
    },

    /// 无效数量
    #[error("无效数量：{0}")]
    InvalidQuantity(String),

    /// 无效价格
    #[error("无效价格：{0}")]
    InvalidPrice(String),

    /// 多币种余额不足
    #[error("多币种余额不足：{0}")]
    MultiCurrencyInsufficient(String),

    /// 更新失败
    #[error("更新失败：{0}")]
    UpdateFailed(String),
}

/// 投资组合 `Result` 别名
pub type PortfolioResult<T> = Result<T, PortfolioError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_not_found_display() {
        let err = PortfolioError::PositionNotFound {
            symbol: Symbol::from("BTC-USDT"),
        };
        assert!(err.to_string().contains("BTC-USDT"));
    }

    #[test]
    fn test_insufficient_cash_display() {
        let err = PortfolioError::InsufficientCash {
            currency: Currency::USD,
            required: 100,
            available: 50,
        };
        let msg = err.to_string();
        assert!(msg.contains("USD"));
        assert!(msg.contains("100"));
        assert!(msg.contains("50"));
    }
}
