//! 组合延迟模型
//!
//! 为不同路径配置不同的子模型，未配置路径回退到默认模型。
//! 内部使用 `Box<dyn LatencyModel>`，因此不实现 `Clone` / `PartialEq` /
//! `Serialize` / `Deserialize`（与 [`crate::impact::AdaptiveImpactModel`] 同因）。

use std::collections::HashMap;
use std::time::Duration;

use super::traits::{LatencyModel, LatencyParams, PathType};

/// 组合延迟模型：不同路径使用不同子模型
pub struct CompositeLatencyModel {
    /// 路径 → 子模型映射
    models: HashMap<PathType, Box<dyn LatencyModel>>,
    /// 默认模型（未指定路径时使用）
    default_model: Box<dyn LatencyModel>,
}

impl std::fmt::Debug for CompositeLatencyModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeLatencyModel")
            .field("default", &self.default_model.name())
            .field(
                "paths",
                &self
                    .models
                    .iter()
                    .map(|(k, v)| (*k, v.name()))
                    .collect::<HashMap<_, _>>(),
            )
            .finish()
    }
}

impl CompositeLatencyModel {
    /// 创建组合模型，初始仅含默认模型
    pub fn new(default: Box<dyn LatencyModel>) -> Self {
        Self {
            models: HashMap::new(),
            default_model: default,
        }
    }

    /// 为特定路径设置子模型
    pub fn with_path(mut self, path: PathType, model: Box<dyn LatencyModel>) -> Self {
        self.models.insert(path, model);
        self
    }

    /// 获取指定路径的子模型数量
    pub fn path_count(&self) -> usize {
        self.models.len()
    }
}

impl LatencyModel for CompositeLatencyModel {
    fn sample_delay(&self, path: PathType) -> Duration {
        match self.models.get(&path) {
            Some(m) => m.sample_delay(path),
            None => self.default_model.sample_delay(path),
        }
    }

    fn name(&self) -> &str {
        "composite"
    }

    fn params(&self) -> LatencyParams {
        LatencyParams {
            model_type: "composite".to_string(),
            base_delay_ms: 0.0,
            jitter_ms: None,
            path_overrides: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::latency::{ConstantLatencyModel, ExponentialLatencyModel};

    #[test]
    fn test_composite_routes_to_path_specific_model() {
        let composite = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )))
        .with_path(
            PathType::OrderSubmit,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(20))),
        );

        // 命中 path-specific
        assert_eq!(
            composite.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(20)
        );
        // 回退到 default
        assert_eq!(
            composite.sample_delay(PathType::MarketData),
            Duration::from_millis(1)
        );
    }

    #[test]
    fn test_composite_mixed_distribution() {
        // 行情用固定（稳定），订单用指数（长尾）
        let composite = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )))
        .with_path(
            PathType::OrderSubmit,
            Box::new(ExponentialLatencyModel::from_mean_ms(5.0)),
        );

        // 行情稳定 = 1ms
        for _ in 0..100 {
            assert_eq!(
                composite.sample_delay(PathType::MarketData),
                Duration::from_millis(1)
            );
        }
        // 订单随机（指数分布，>= 0）
        for _ in 0..100 {
            let d = composite.sample_delay(PathType::OrderSubmit);
            assert!(d.as_secs_f64() >= 0.0);
        }
    }

    #[test]
    fn test_composite_path_count() {
        let c = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )))
        .with_path(
            PathType::OrderSubmit,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(2))),
        )
        .with_path(
            PathType::OrderCancel,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(3))),
        );
        assert_eq!(c.path_count(), 2);
    }

    #[test]
    fn test_composite_name_and_params() {
        let c = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )));
        assert_eq!(c.name(), "composite");
        let p = c.params();
        assert_eq!(p.model_type, "composite");
    }

    #[test]
    fn test_composite_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CompositeLatencyModel>();
    }
}
