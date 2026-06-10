//! 观测空间错误类型

use thiserror::Error;

/// 观测空间错误
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ObservationError {
    /// 特征名未在 `MarketState` 中找到
    #[error("Feature '{feature}' not found in feature set")]
    FeatureNotFound {
        /// 缺失的特征名
        feature: String,
    },

    /// 窗口大小非法（必须 > 0）
    #[error("Window size must be > 0, got {0}")]
    InvalidWindowSize(usize),

    /// 特征数量与配置不匹配
    #[error("Feature count mismatch: expected {expected}, got {actual}")]
    FeatureCountMismatch {
        /// 期望数量
        expected: usize,
        /// 实际数量
        actual: usize,
    },

    /// 归一化失败
    #[error("Normalization failed: {reason}")]
    NormalizationFailed {
        /// 失败原因
        reason: String,
    },

    /// 数据不足
    #[error("Insufficient data: need {needed} ticks, have {have}")]
    InsufficientData {
        /// 所需数据量
        needed: usize,
        /// 实际数据量
        have: usize,
    },
}

/// 观测空间统一 Result 类型
pub type ObservationResult<T> = Result<T, ObservationError>;

/// 验证观测空间配置
///
/// - 窗口大小必须 > 0
/// - 特征列表非空
/// - 特征名唯一
pub fn validate_observation_space(
    features: &[FeatureConfig],
    window_size: usize,
) -> ObservationResult<()> {
    use std::collections::HashSet;

    if features.is_empty() {
        return Err(ObservationError::FeatureCountMismatch {
            expected: 1,
            actual: 0,
        });
    }
    if window_size == 0 {
        return Err(ObservationError::InvalidWindowSize(window_size));
    }

    let mut names = HashSet::new();
    for feat in features {
        if !names.insert(&feat.name) {
            return Err(ObservationError::NormalizationFailed {
                reason: format!("Duplicate feature name: {}", feat.name),
            });
        }
    }
    Ok(())
}

use crate::observation::types::FeatureConfig;
