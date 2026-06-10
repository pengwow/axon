//! 撮合引擎错误

use thiserror::Error;

use axon_core::types::{Price, Quantity};

/// 撮合引擎错误
#[derive(Debug, Clone, Error, PartialEq)]
pub enum MatchingError {
    /// 订单未找到
    #[error("订单未找到: {order_id}")]
    OrderNotFound {
        /// 订单 ID
        order_id: u64,
    },

    /// 无效修改
    #[error("无效修改: {reason}")]
    InvalidModification {
        /// 修改原因
        reason: String,
    },

    /// 订单已完全成交
    #[error("订单已完全成交")]
    OrderAlreadyFilled,

    /// 价格必须大于零
    #[error("价格必须大于零: {price}")]
    InvalidPrice {
        /// 实际价格
        price: Price,
    },

    /// 数量必须大于零
    #[error("数量必须大于零: {quantity}")]
    InvalidQuantity {
        /// 实际数量
        quantity: Quantity,
    },

    /// 订单簿为空
    #[error("订单簿为空: {side:?}")]
    OrderBookEmpty {
        /// 询价方向
        side: axon_core::market::Side,
    },

    /// FOK 订单无法全部成交
    #[error("FOK 订单无法全部成交: 需要 {required}, 可用 {available}")]
    FokPartialFill {
        /// 所需数量
        required: Quantity,
        /// 可用数量
        available: Quantity,
    },

    /// 不支持的订单类型
    #[error("撮合引擎 L1 不支持订单类型: {0}")]
    UnsupportedOrderType(String),
}

/// 撮合引擎 `Result` 别名
pub type MatchingResult<T> = std::result::Result<T, MatchingError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MatchingError::OrderNotFound { order_id: 42 };
        assert!(err.to_string().contains("42"));
    }

    #[test]
    fn test_error_order_already_filled() {
        let err = MatchingError::OrderAlreadyFilled;
        assert!(err.to_string().contains("完全成交"));
    }
}
