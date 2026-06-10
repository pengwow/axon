//! 统一错误类型（占位实现，Phase 1A 阶段完善）

use thiserror::Error;

/// AXON 核心错误类型
#[derive(Debug, Error)]
pub enum Error {
    /// 通用错误，用于占位与尚未分类的失败
    #[error("core error: {0}")]
    Other(String),
}

/// 核心 crate 的 `Result` 别名
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_includes_context() {
        let err = Error::Other("invalid timestamp".to_string());
        assert_eq!(err.to_string(), "core error: invalid timestamp");
    }
}
