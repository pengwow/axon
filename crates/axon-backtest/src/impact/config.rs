//! [`ImpactedMatchingEngine`] 的 TOML 配置
//!
//! 支持从 TOML 文件/字符串加载冲击模型配置（含永久冲击衰减、深度等参数）。
//!
//! # 示例 TOML
//!
//! ```toml
//! [model]
//! type = "linear"           # "linear" | "power_law"
//! coefficient = 0.05
//! depth_levels = 10
//! instantaneous_ratio = 0.7
//!
//! # 仅 PowerLaw 必需
//! # exponent = 0.5
//!
//! [permanent]
//! decay = 0.0               # 0.0~1.0, 0 = 不衰减
//! ```
//!
//! # 加载
//!
//! ```no_run
//! use axon_backtest::impact::ImpactedEngineConfig;
//!
//! let toml_str = r#"
//! [model]
//! type = "linear"
//! coefficient = 0.05
//! depth_levels = 5
//! instantaneous_ratio = 0.7
//! "#;
//!
//! let config = ImpactedEngineConfig::from_toml(toml_str).unwrap();
//! let engine = config.build_engine();
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

use axon_core::impact::{ImpactModelConfig, create_model};

use crate::impact::impacted_engine::ImpactedMatchingEngine;

/// 模型类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    /// 线性冲击
    #[default]
    Linear,
    /// 幂律冲击
    PowerLaw,
}

/// 冲击模型配置子表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 模型类型
    #[serde(default = "default_model_type")]
    pub r#type: ModelType,
    /// 冲击系数
    pub coefficient: f64,
    /// 深度层级数
    #[serde(default = "default_depth_levels")]
    pub depth_levels: usize,
    /// 即时/永久冲击比例
    #[serde(default = "default_instantaneous_ratio")]
    pub instantaneous_ratio: f64,
    /// 幂律指数（仅 PowerLaw 必需）
    #[serde(default = "default_exponent")]
    pub exponent: f64,
}

fn default_model_type() -> ModelType {
    ModelType::Linear
}
fn default_depth_levels() -> usize {
    10
}
fn default_instantaneous_ratio() -> f64 {
    0.7
}
fn default_exponent() -> f64 {
    0.5
}

impl ModelConfig {
    /// 转换为 [`ImpactModelConfig`]（来自 axon-core）
    pub fn to_impact_model_config(&self) -> ImpactModelConfig {
        match self.r#type {
            ModelType::Linear => ImpactModelConfig::Linear {
                coefficient: self.coefficient,
                depth_levels: self.depth_levels,
                instantaneous_ratio: self.instantaneous_ratio,
            },
            ModelType::PowerLaw => ImpactModelConfig::PowerLaw {
                coefficient: self.coefficient,
                exponent: self.exponent,
                depth_levels: self.depth_levels,
                instantaneous_ratio: self.instantaneous_ratio,
            },
        }
    }
}

/// 永久冲击配置子表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermanentConfig {
    /// 衰减率（0.0~1.0，0.0 = 不衰减）
    #[serde(default)]
    pub decay: f64,
}

impl Default for PermanentConfig {
    fn default() -> Self {
        Self { decay: 0.0 }
    }
}

/// 完整引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactedEngineConfig {
    /// 冲击模型配置
    pub model: ModelConfig,
    /// 永久冲击配置
    #[serde(default)]
    pub permanent: PermanentConfig,
}

impl Default for ImpactedEngineConfig {
    fn default() -> Self {
        Self {
            model: ModelConfig {
                r#type: ModelType::Linear,
                coefficient: 0.05,
                depth_levels: 10,
                instantaneous_ratio: 0.7,
                exponent: 0.5,
            },
            permanent: PermanentConfig::default(),
        }
    }
}

impl ImpactedEngineConfig {
    /// 从 TOML 字符串加载
    pub fn from_toml(toml_str: &str) -> Result<Self, ImpactedConfigError> {
        toml::from_str(toml_str).map_err(ImpactedConfigError::Toml)
    }

    /// 从 TOML 文件加载
    pub fn from_toml_file(path: &std::path::Path) -> Result<Self, ImpactedConfigError> {
        let content = std::fs::read_to_string(path).map_err(ImpactedConfigError::Io)?;
        Self::from_toml(&content)
    }

    /// 构造 [`ImpactedMatchingEngine`]
    pub fn build_engine(&self) -> ImpactedMatchingEngine {
        let impact_config = self.model.to_impact_model_config();
        let engine = ImpactedMatchingEngine::from_config(impact_config);
        if self.permanent.decay > 0.0 {
            engine.with_permanent_decay(self.permanent.decay)
        } else {
            engine
        }
    }

    /// 构造 `Box<dyn ImpactModel>`（便于复用）
    pub fn build_model(&self) -> Box<dyn axon_core::impact::ImpactModel> {
        create_model(self.model.to_impact_model_config())
    }

    /// 校验配置合法性
    pub fn validate(&self) -> Result<(), ImpactedConfigError> {
        // 系数
        if !self.model.coefficient.is_finite() || self.model.coefficient < 0.0 {
            return Err(ImpactedConfigError::InvalidModel(format!(
                "coefficient 必须为非负有限数，实际 {}",
                self.model.coefficient
            )));
        }
        // 深度
        if self.model.depth_levels == 0 {
            return Err(ImpactedConfigError::InvalidModel(
                "depth_levels 必须 > 0".to_string(),
            ));
        }
        // 即时/永久比例
        if !(0.0..=1.0).contains(&self.model.instantaneous_ratio) {
            return Err(ImpactedConfigError::InvalidModel(format!(
                "instantaneous_ratio 必须在 [0, 1]，实际 {}",
                self.model.instantaneous_ratio
            )));
        }
        // 幂律指数
        if matches!(self.model.r#type, ModelType::PowerLaw)
            && !(self.model.exponent > 0.0 && self.model.exponent <= 2.0)
        {
            return Err(ImpactedConfigError::InvalidModel(format!(
                "PowerLaw exponent 必须在 (0, 2]，实际 {}",
                self.model.exponent
            )));
        }
        // 衰减
        if !(0.0..=1.0).contains(&self.permanent.decay) {
            return Err(ImpactedConfigError::InvalidModel(format!(
                "permanent.decay 必须在 [0, 1]，实际 {}",
                self.permanent.decay
            )));
        }
        Ok(())
    }
}

/// 加载错误
#[derive(Debug, Error)]
pub enum ImpactedConfigError {
    /// TOML 解析错误
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// IO 错误
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// 模型校验错误
    #[error("invalid model: {0}")]
    InvalidModel(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validates() {
        let cfg = ImpactedEngineConfig::default();
        cfg.validate().expect("default should validate");
    }

    #[test]
    fn test_from_toml_linear() {
        let toml_str = r#"
[model]
type = "linear"
coefficient = 0.05
depth_levels = 5
instantaneous_ratio = 0.7
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        assert_eq!(cfg.model.r#type, ModelType::Linear);
        assert_eq!(cfg.model.coefficient, 0.05);
        assert_eq!(cfg.model.depth_levels, 5);
        assert!((cfg.model.instantaneous_ratio - 0.7).abs() < 1e-9);
    }

    #[test]
    fn test_from_toml_power_law() {
        let toml_str = r#"
[model]
type = "power_law"
coefficient = 0.1
exponent = 0.6
depth_levels = 8
instantaneous_ratio = 0.65
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        assert_eq!(cfg.model.r#type, ModelType::PowerLaw);
        assert!((cfg.model.exponent - 0.6).abs() < 1e-9);
    }

    #[test]
    fn test_from_toml_with_permanent_decay() {
        let toml_str = r#"
[model]
type = "linear"
coefficient = 0.05

[permanent]
decay = 0.1
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        assert!((cfg.permanent.decay - 0.1).abs() < 1e-9);
    }

    #[test]
    fn test_from_toml_uses_defaults() {
        let toml_str = r#"
[model]
type = "linear"
coefficient = 0.05
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        assert_eq!(cfg.model.depth_levels, 10);
        assert!((cfg.model.instantaneous_ratio - 0.7).abs() < 1e-9);
    }

    #[test]
    fn test_validate_negative_coefficient() {
        let mut cfg = ImpactedEngineConfig::default();
        cfg.model.coefficient = -0.1;
        let result = cfg.validate();
        assert!(matches!(result, Err(ImpactedConfigError::InvalidModel(_))));
    }

    #[test]
    fn test_validate_zero_depth() {
        let mut cfg = ImpactedEngineConfig::default();
        cfg.model.depth_levels = 0;
        let result = cfg.validate();
        assert!(matches!(result, Err(ImpactedConfigError::InvalidModel(_))));
    }

    #[test]
    fn test_validate_ratio_out_of_range() {
        let mut cfg = ImpactedEngineConfig::default();
        cfg.model.instantaneous_ratio = 1.5;
        let result = cfg.validate();
        assert!(matches!(result, Err(ImpactedConfigError::InvalidModel(_))));
    }

    #[test]
    fn test_validate_power_law_exponent_out_of_range() {
        let mut cfg = ImpactedEngineConfig::default();
        cfg.model.r#type = ModelType::PowerLaw;
        cfg.model.exponent = 2.5;
        let result = cfg.validate();
        assert!(matches!(result, Err(ImpactedConfigError::InvalidModel(_))));
    }

    #[test]
    fn test_validate_decay_out_of_range() {
        let mut cfg = ImpactedEngineConfig::default();
        cfg.permanent.decay = 1.5;
        let result = cfg.validate();
        assert!(matches!(result, Err(ImpactedConfigError::InvalidModel(_))));
    }

    #[test]
    fn test_build_engine_linear() {
        let toml_str = r#"
[model]
type = "linear"
coefficient = 0.05
depth_levels = 5
instantaneous_ratio = 0.7
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        let engine = cfg.build_engine();
        assert_eq!(engine.model_name(), "LinearImpact");
    }

    #[test]
    fn test_build_engine_power_law() {
        let toml_str = r#"
[model]
type = "power_law"
coefficient = 0.1
exponent = 0.5
depth_levels = 10
instantaneous_ratio = 0.7
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        let engine = cfg.build_engine();
        assert_eq!(engine.model_name(), "PowerLawImpact");
    }

    #[test]
    fn test_build_engine_with_decay() {
        let toml_str = r#"
[model]
type = "linear"
coefficient = 0.05

[permanent]
decay = 0.2
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        let engine = cfg.build_engine();
        assert_eq!(engine.permanent_decay(), Some(0.2));
    }

    #[test]
    fn test_build_engine_without_decay() {
        let toml_str = r#"
[model]
type = "linear"
coefficient = 0.05
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        let engine = cfg.build_engine();
        assert_eq!(engine.permanent_decay(), None);
    }

    #[test]
    fn test_to_impact_model_config_linear() {
        let cfg = ModelConfig {
            r#type: ModelType::Linear,
            coefficient: 0.05,
            depth_levels: 5,
            instantaneous_ratio: 0.7,
            exponent: 0.5,
        };
        let impact_cfg = cfg.to_impact_model_config();
        assert!(matches!(impact_cfg, ImpactModelConfig::Linear { .. }));
    }

    #[test]
    fn test_to_impact_model_config_power_law() {
        let cfg = ModelConfig {
            r#type: ModelType::PowerLaw,
            coefficient: 0.1,
            depth_levels: 5,
            instantaneous_ratio: 0.7,
            exponent: 0.6,
        };
        let impact_cfg = cfg.to_impact_model_config();
        match impact_cfg {
            ImpactModelConfig::PowerLaw { exponent, .. } => {
                assert!((exponent - 0.6).abs() < 1e-9);
            }
            _ => panic!("expected PowerLaw"),
        }
    }

    #[test]
    fn test_build_model_returns_dyn_object() {
        let cfg = ImpactedEngineConfig::default();
        let m = cfg.build_model();
        assert_eq!(m.name(), "LinearImpact");
    }

    #[test]
    fn test_serialize_roundtrip() {
        let toml_str = r#"
[model]
type = "linear"
coefficient = 0.05
depth_levels = 5
instantaneous_ratio = 0.7

[permanent]
decay = 0.1
"#;
        let cfg = ImpactedEngineConfig::from_toml(toml_str).unwrap();
        let json = serde_json::to_string(&cfg).unwrap();
        let de: ImpactedEngineConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg.model.coefficient, de.model.coefficient);
        assert!((cfg.permanent.decay - de.permanent.decay).abs() < 1e-9);
    }

    #[test]
    fn test_default_config_serializes() {
        let cfg = ImpactedEngineConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("linear"));
        assert!(json.contains("0.05"));
    }

    // ─── LinearImpactModel / PowerLawImpactModel 验证 ───

    #[test]
    fn test_linear_model_construction() {
        // 不通过 toml，直接验证 ModelConfig 字段
        let cfg = ModelConfig {
            r#type: ModelType::Linear,
            coefficient: 0.05,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
            exponent: 0.5,
        };
        assert!(matches!(cfg.r#type, ModelType::Linear));
    }

    #[test]
    fn test_power_law_model_construction() {
        let cfg = ModelConfig {
            r#type: ModelType::PowerLaw,
            coefficient: 0.1,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
            exponent: 0.5,
        };
        assert!(matches!(cfg.r#type, ModelType::PowerLaw));
    }

    // 抑制未使用导入警告
    #[allow(dead_code)]
    fn _use_types() {
        let _m: Box<dyn axon_core::impact::ImpactModel> =
            Box::new(axon_core::impact::LinearImpactModel::new(0.05));
    }
}
