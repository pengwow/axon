//! EnsembleError 单元测试

use axon_ensemble::EnsembleError;

#[test]
fn test_no_models_error_is_not_recoverable() {
    // NoModels 是结构性错误，需要外部修复
    let err = EnsembleError::NoModels;
    assert!(!err.is_recoverable());
    assert!(err.to_string().contains("模型数量为零"));
}

#[test]
fn test_prediction_failed_is_recoverable() {
    // 单个模型失败可重试
    let err = EnsembleError::PredictionFailed {
        model_name: "ppo_v1".to_string(),
    };
    assert!(err.is_recoverable());
}

#[test]
fn test_weight_mismatch_is_recoverable() {
    // 权重不匹配可重试
    let err = EnsembleError::WeightMismatch {
        expected: 3,
        actual: 2,
    };
    assert!(err.is_recoverable());
}

#[test]
fn test_invalid_weights_is_not_recoverable() {
    // 权重和不为 1 是配置错误，不可恢复
    let err = EnsembleError::InvalidWeights { sum: 0.5 };
    assert!(!err.is_recoverable());
}

#[test]
fn test_meta_model_failed_is_not_recoverable() {
    // 元模型失败是系统性错误，不可恢复
    let err = EnsembleError::MetaModelFailed("inference timeout".to_string());
    assert!(!err.is_recoverable());
}

#[test]
fn test_error_display_messages() {
    assert_eq!(
        EnsembleError::WeightMismatch {
            expected: 3,
            actual: 2
        }
        .to_string(),
        "权重数量不匹配: 期望 3, 实际 2"
    );
    assert_eq!(
        EnsembleError::InvalidWeights { sum: 0.7 }.to_string(),
        "权重和不为一: 0.7"
    );
}
