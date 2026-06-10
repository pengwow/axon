//! 订单模块错误

use thiserror::Error;

use super::status::OrderStatus;
use crate::types::Quantity;

/// 订单模块错误
#[derive(Debug, Clone, Error)]
pub enum OrderError {
    /// 非法状态转换
    #[error("无效状态转换：{from:?} -> {to:?}")]
    InvalidStateTransition {
        /// 起始状态
        from: OrderStatus,
        /// 目标状态
        to: OrderStatus,
    },

    /// 订单未处于活跃状态
    #[error("订单已{status}，无法执行操作")]
    OrderNotActive {
        /// 当前状态
        status: OrderStatus,
    },

    /// 成交量超出订单剩余量
    #[error("成交数量({filled})超出订单剩余量({remaining})")]
    OverFill {
        /// 本次成交量
        filled: Quantity,
        /// 订单剩余量
        remaining: Quantity,
    },

    /// FOK 订单部分成交（应整单取消）
    #[error("FOK 订单部分成交({filled}/{total})，已全部取消")]
    FokPartialFill {
        /// 已成交量
        filled: Quantity,
        /// 订单总量
        total: Quantity,
    },

    /// IOC 订单部分成交（剩余已取消）
    #[error("IOC 订单部分成交({filled}/{total})，剩余已取消")]
    IocPartialFill {
        /// 已成交量
        filled: Quantity,
        /// 订单总量
        total: Quantity,
    },

    /// 订单已过期
    #[error("订单已过期")]
    Expired,

    /// 订单已被取消
    #[error("订单已被取消")]
    Cancelled,
}

/// 订单模块的 `Result` 别名
pub type OrderResult<T> = std::result::Result<T, OrderError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_error_display() {
        let err = OrderError::Expired;
        assert_eq!(err.to_string(), "订单已过期");

        let err = OrderError::Cancelled;
        assert_eq!(err.to_string(), "订单已被取消");
    }
}
