//! 费用模型错误类型

use thiserror::Error;

/// 费用模型模块错误
#[derive(Debug, Clone, Error, PartialEq)]
pub enum FeeModelError {
    /// 交易所未注册
    #[error("交易所未注册：{0}")]
    ExchangeNotRegistered(String),

    /// 未配置费率阶梯
    #[error("未配置费率阶梯：{0}")]
    NoTiersConfigured(String),

    /// 无效的费率
    #[error("无效的费率：{0}")]
    InvalidRate(String),

    /// 无效的数量
    #[error("无效的数量：{0}")]
    InvalidQuantity(String),

    /// 计算溢出
    #[error("费用计算溢出：{0}")]
    Overflow(String),
}

/// 费用模型 `Result` 别名
pub type FeeModelResult<T> = std::result::Result<T, FeeModelError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exchange_not_registered_display() {
        let err = FeeModelError::ExchangeNotRegistered("binance".into());
        let msg = err.to_string();
        assert!(msg.contains("未注册"));
        assert!(msg.contains("binance"));
    }

    #[test]
    fn test_no_tiers_configured_display() {
        let err = FeeModelError::NoTiersConfigured("kraken".into());
        let msg = err.to_string();
        assert!(msg.contains("未配置费率阶梯"));
        assert!(msg.contains("kraken"));
    }

    #[test]
    fn test_invalid_rate_display() {
        let err = FeeModelError::InvalidRate("-0.001".into());
        let msg = err.to_string();
        assert!(msg.contains("无效的费率"));
        assert!(msg.contains("-0.001"));
    }

    #[test]
    fn test_invalid_quantity_display() {
        let err = FeeModelError::InvalidQuantity("负数".into());
        let msg = err.to_string();
        assert!(msg.contains("无效的数量"));
        assert!(msg.contains("负数"));
    }

    #[test]
    fn test_overflow_display() {
        let err = FeeModelError::Overflow("1e18 * 0.001".into());
        let msg = err.to_string();
        assert!(msg.contains("费用计算溢出"));
        assert!(msg.contains("1e18"));
    }

    /// 错误类型 Clone 一致性
    #[test]
    fn test_error_clone_preserves_variant() {
        let err = FeeModelError::ExchangeNotRegistered("x".into());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    /// 不同 variant 不相等
    #[test]
    fn test_error_variants_not_equal() {
        let a = FeeModelError::ExchangeNotRegistered("x".into());
        let b = FeeModelError::NoTiersConfigured("x".into());
        assert_ne!(a, b);
    }

    /// 空字符串
    #[test]
    fn test_error_empty_string_payload() {
        let err = FeeModelError::ExchangeNotRegistered(String::new());
        assert!(err.to_string().contains("未注册"));
    }
}
