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

    // ─── 边界测试 ──────────────────────────────────────────

    /// 零延迟 default + 零延迟 path ⇒ 全部返回 0
    #[test]
    fn test_all_zero_delays() {
        let composite =
            CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(Duration::ZERO)))
                .with_path(
                    PathType::OrderSubmit,
                    Box::new(ConstantLatencyModel::uniform(Duration::ZERO)),
                );

        for path in PathType::ALL {
            assert_eq!(composite.sample_delay(path), Duration::ZERO);
        }
    }

    /// 全部 5 个路径都使用不同子模型
    #[test]
    fn test_all_paths_overridden() {
        let composite = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )))
        .with_path(
            PathType::MarketData,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(2))),
        )
        .with_path(
            PathType::OrderSubmit,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(3))),
        )
        .with_path(
            PathType::OrderCancel,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(4))),
        )
        .with_path(
            PathType::AccountQuery,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(5))),
        )
        .with_path(
            PathType::Heartbeat,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(6))),
        );

        assert_eq!(
            composite.sample_delay(PathType::MarketData),
            Duration::from_millis(2)
        );
        assert_eq!(
            composite.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(3)
        );
        assert_eq!(
            composite.sample_delay(PathType::OrderCancel),
            Duration::from_millis(4)
        );
        assert_eq!(
            composite.sample_delay(PathType::AccountQuery),
            Duration::from_millis(5)
        );
        assert_eq!(
            composite.sample_delay(PathType::Heartbeat),
            Duration::from_millis(6)
        );
        assert_eq!(composite.path_count(), 5);
    }

    /// 同一路径多次 with_path ⇒ 后者覆盖前者
    #[test]
    fn test_path_override_chain() {
        let composite = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )))
        .with_path(
            PathType::OrderSubmit,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(10))),
        )
        .with_path(
            PathType::OrderSubmit,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(20))),
        );

        // 后者覆盖
        assert_eq!(
            composite.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(20)
        );
        // path_count 仍为 1（同一 key）
        assert_eq!(composite.path_count(), 1);
    }

    /// 嵌套：Composite 作为 default
    #[test]
    fn test_nested_composite() {
        let inner = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )))
        .with_path(
            PathType::MarketData,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(5))),
        );

        let outer = CompositeLatencyModel::new(Box::new(inner)).with_path(
            PathType::OrderSubmit,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(50))),
        );

        // MarketData 应回退到 inner（再回退到 inner.default = 1ms...不对）
        // outer 没有 MarketData ⇒ 回退到 inner ⇒ inner 有 MarketData = 5ms
        assert_eq!(
            outer.sample_delay(PathType::MarketData),
            Duration::from_millis(5)
        );
        assert_eq!(
            outer.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(50)
        );
    }

    /// path_count 初始为 0
    #[test]
    fn test_path_count_zero_initially() {
        let c = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )));
        assert_eq!(c.path_count(), 0);
    }

    /// Debug 输出包含 default 和 paths
    #[test]
    fn test_debug_contains_default_and_paths() {
        let c = CompositeLatencyModel::new(Box::new(ConstantLatencyModel::uniform(
            Duration::from_millis(1),
        )))
        .with_path(
            PathType::OrderSubmit,
            Box::new(ConstantLatencyModel::uniform(Duration::from_millis(10))),
        );
        let s = format!("{c:?}");
        assert!(s.contains("CompositeLatencyModel"));
        assert!(s.contains("default"));
        assert!(s.contains("paths"));
        assert!(s.contains("constant"));
    }
}
