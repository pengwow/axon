//! 统一错误类型

use thiserror::Error;

use crate::types::ModelStage;

/// 注册表错误
#[derive(Debug, Error)]
pub enum RegistryError {
    /// 模型未找到
    #[error("model not found: {0}")]
    ModelNotFound(String),

    /// 版本未找到
    #[error("version not found: {0}@{1}")]
    VersionNotFound(String, String),

    /// 当前无 Production 版本
    #[error("no production version: {0}")]
    NoProductionVersion(String),

    /// 无效版本号
    #[error("invalid version: {0}")]
    InvalidVersion(String),

    /// 阶段转换非法
    #[error("invalid stage transition: {from:?} -> {to:?}")]
    InvalidTransition {
        /// 起始阶段
        from: ModelStage,
        /// 目标阶段
        to: ModelStage,
    },

    /// 存储错误
    #[error("storage error: {0}")]
    StorageError(String),

    /// 产物未找到
    #[error("artifact not found: {0}")]
    ArtifactNotFound(String),

    /// 配置错误
    #[error("config error: {0}")]
    ConfigError(String),

    /// 回滚失败
    #[error("rollback failed: {0}")]
    RollbackFailed(String),

    /// 索引损坏
    #[error("index corrupted: {0}")]
    IndexCorrupted(String),

    /// IO 错误
    #[error("io error: {0}")]
    Io(String),

    /// 序列化错误
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl RegistryError {
    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::StorageError(_) | Self::Io(_))
    }
}

/// Registry Result 类型别名
pub type RegistryResult<T> = Result<T, RegistryError>;
