//! 错误类型

use thiserror::Error;

/// 冲击模型模块错误
#[derive(Debug, Clone, Error, PartialEq)]
pub enum ImpactModelError {
    /// 订单簿为空
    #[error("订单簿为空，无法计算冲击")]
    EmptyOrderBook,

    /// 无效参数
    #[error("无效参数：{0}")]
    InvalidParameter(String),

    /// 深度不足
    #[error("深度不足：需要 {required} 层，实际 {available} 层")]
    InsufficientDepth {
        /// 需要的层数
        required: usize,
        /// 实际可用层数
        available: usize,
    },

    /// 计算溢出
    #[error("计算溢出")]
    ComputationOverflow,
}

/// 冲击模型 `Result` 别名
pub type ImpactModelResult<T> = std::result::Result<T, ImpactModelError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_orderbook_display() {
        let err = ImpactModelError::EmptyOrderBook;
        assert!(err.to_string().contains("订单簿为空"));
    }

    #[test]
    fn test_invalid_parameter_display() {
        let err = ImpactModelError::InvalidParameter("coefficient < 0".into());
        assert!(err.to_string().contains("coefficient"));
    }

    #[test]
    fn test_insufficient_depth_display() {
        let err = ImpactModelError::InsufficientDepth {
            required: 10,
            available: 5,
        };
        let msg = err.to_string();
        assert!(msg.contains("10"));
        assert!(msg.contains("5"));
    }
}
