//! 市场数据错误类型

use thiserror::Error;

use super::orderbook::OrderBookLevel;
use crate::types::Price;

/// 市场数据错误
#[derive(Debug, Clone, Error)]
pub enum MarketDataError {
    /// OHLC 数据不一致
    #[error("OHLC 不一致：high({high}) < low({low})")]
    OhlcInconsistent {
        /// 最高价
        high: Price,
        /// 最低价
        low: Price,
    },

    /// 无效价格
    #[error("无效价格：{0}")]
    InvalidPrice(String),

    /// 无效数量
    #[error("无效数量：{0}")]
    InvalidQuantity(String),

    /// 订单簿为空
    #[error("订单簿为空，无法计算 {0}")]
    OrderBookEmpty(String),

    /// 订单簿未排序
    #[error("订单簿未排序：{bid_level:?} > {ask_level:?}")]
    OrderBookUnsorted {
        /// 错误的买价层
        bid_level: OrderBookLevel,
        /// 错误的卖价层
        ask_level: OrderBookLevel,
    },
}

/// 市场数据模块的 `Result` 别名
pub type MarketDataResult<T> = std::result::Result<T, MarketDataError>;
