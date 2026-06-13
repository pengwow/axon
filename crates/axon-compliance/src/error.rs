//! 合规审计错误类型

use thiserror::Error;

/// 合规审计错误
#[derive(Error, Debug)]
pub enum ComplianceError {
    /// 无效的交易数据
    #[error("Invalid trade data: {0}")]
    InvalidTradeData(String),

    /// 持仓集中度限制被突破
    #[error(
        "Concentration limit breached: {symbol} current {current_pct:.2}%, limit {limit_pct:.2}%"
    )]
    ConcentrationLimitBreached {
        symbol: String,
        current_pct: f64,
        limit_pct: f64,
    },

    /// 大额交易阈值被超过
    #[error("Large trade threshold exceeded: {notional:.2} > {threshold:.2}")]
    LargeTradeThresholdExceeded { notional: f64, threshold: f64 },

    /// 审计日志完整性检查失败
    #[error("Audit log integrity check failed")]
    AuditIntegrityFailed,

    /// 存储错误
    #[error("Storage error: {0}")]
    StorageError(String),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// 报告生成错误
    #[error("Report generation error: {0}")]
    ReportError(String),

    /// 监管格式错误
    #[error("Regulator format error: {0}")]
    RegulatorFormatError(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// 合规审计结果类型
pub type ComplianceResult<T> = Result<T, ComplianceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ComplianceError::InvalidTradeData("quantity must be positive".into());
        assert!(err.to_string().contains("quantity must be positive"));
    }

    #[test]
    fn test_concentration_error_display() {
        let err = ComplianceError::ConcentrationLimitBreached {
            symbol: "BTCUSDT".into(),
            current_pct: 45.5,
            limit_pct: 40.0,
        };
        let msg = err.to_string();
        assert!(msg.contains("BTCUSDT"));
        assert!(msg.contains("45.50%"));
        assert!(msg.contains("40.00%"));
    }
}
