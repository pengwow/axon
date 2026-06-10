//! 冲击模型 trait

use super::linear::LinearImpactModel;
use super::power_law::PowerLawImpactModel;
use super::types::{Impact, ImpactModelConfig};
use crate::market::{OrderBookSnapshot, Side};
use crate::types::Quantity;

/// 冲击模型 trait
///
/// 实现方需提供 `compute_impact` 与元数据。
pub trait ImpactModel: Send + Sync {
    /// 计算市场冲击
    fn compute_impact(
        &self,
        order_quantity: Quantity,
        side: Side,
        order_book: &OrderBookSnapshot,
    ) -> Impact;

    /// 模型名称
    fn name(&self) -> &str;

    /// 模型参数摘要
    fn params(&self) -> String;
}

// ─── 工厂函数 ──────────────────────────────────────────────

/// 创建默认线性冲击模型（coefficient = 0.05）
pub fn linear_impact() -> LinearImpactModel {
    LinearImpactModel::default()
}

/// 创建默认幂律冲击模型（square-root law：coefficient = 0.1, exponent = 0.5）
pub fn sqrt_impact() -> PowerLawImpactModel {
    PowerLawImpactModel::default()
}

/// 根据配置创建模型
pub fn create_model(config: ImpactModelConfig) -> Box<dyn ImpactModel> {
    match config {
        ImpactModelConfig::Linear {
            coefficient,
            depth_levels,
            instantaneous_ratio,
        } => Box::new(
            LinearImpactModel::new(coefficient)
                .with_depth(depth_levels)
                .with_instantaneous_ratio(instantaneous_ratio),
        ),
        ImpactModelConfig::PowerLaw {
            coefficient,
            exponent,
            depth_levels,
            instantaneous_ratio,
        } => Box::new(
            PowerLawImpactModel::new(coefficient, exponent)
                .with_depth(depth_levels)
                .with_instantaneous_ratio(instantaneous_ratio),
        ),
    }
}

// 抑制 unused 警告（在 mod.rs 中通过 pub use re-export 重新导出）
#[allow(dead_code)]
fn _assert_send_sync<T: Send + Sync>() {}

#[allow(dead_code)]
fn _assert_impact_model_send_sync<T: ImpactModel>() {
    _assert_send_sync::<T>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::impact::AdaptiveImpactModel;
    use crate::market::{OrderBookLevel, OrderBookSnapshot};
    use crate::time::Timestamp;
    use crate::types::{Price, Quantity};

    fn sample_ob() -> OrderBookSnapshot {
        OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![OrderBookLevel {
                price: Price::from_f64(99.0),
                quantity: Quantity::from_f64(100.0),
            }],
            asks: vec![OrderBookLevel {
                price: Price::from_f64(100.0),
                quantity: Quantity::from_f64(100.0),
            }],
        }
    }

    #[test]
    fn test_create_model_linear() {
        let m: Box<dyn ImpactModel> = create_model(ImpactModelConfig::Linear {
            coefficient: 0.05,
            depth_levels: 5,
            instantaneous_ratio: 0.7,
        });
        assert_eq!(m.name(), "LinearImpact");
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        // 0.05 × (10 / 100) = 0.005
        assert!((impact.total() - 0.005).abs() < 1e-10);
    }

    #[test]
    fn test_create_model_power_law() {
        let m: Box<dyn ImpactModel> = create_model(ImpactModelConfig::PowerLaw {
            coefficient: 0.1,
            exponent: 0.5,
            depth_levels: 5,
            instantaneous_ratio: 0.7,
        });
        assert_eq!(m.name(), "PowerLawImpact");
    }

    #[test]
    fn test_linear_impact_factory() {
        let m = linear_impact();
        assert_eq!(m.name(), "LinearImpact");
    }

    #[test]
    fn test_sqrt_impact_factory() {
        let m = sqrt_impact();
        assert_eq!(m.name(), "PowerLawImpact");
        assert!(m.params().contains("exponent=0.5"));
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 工厂传入零 coefficient 应创建合法模型（零冲击）
    #[test]
    fn test_factory_linear_zero_coefficient() {
        let m: Box<dyn ImpactModel> = create_model(ImpactModelConfig::Linear {
            coefficient: 0.0,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
        });
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert_eq!(impact, Impact::zero());
    }

    /// 工厂传入负 ratio 应 panic
    #[test]
    #[should_panic(expected = "必须在 [0, 1] 范围")]
    fn test_factory_linear_negative_ratio_panics() {
        let _: Box<dyn ImpactModel> = create_model(ImpactModelConfig::Linear {
            coefficient: 0.05,
            depth_levels: 10,
            instantaneous_ratio: -0.1,
        });
    }

    /// 工厂传入 ratio > 1.0 应 panic
    #[test]
    #[should_panic(expected = "必须在 [0, 1] 范围")]
    fn test_factory_linear_ratio_too_large_panics() {
        let _: Box<dyn ImpactModel> = create_model(ImpactModelConfig::Linear {
            coefficient: 0.05,
            depth_levels: 10,
            instantaneous_ratio: 1.5,
        });
    }

    /// 工厂传入 ratio = 0 / 1.0 边界
    #[test]
    fn test_factory_linear_ratio_boundary_ok() {
        for ratio in [0.0_f64, 1.0_f64] {
            let m: Box<dyn ImpactModel> = create_model(ImpactModelConfig::Linear {
                coefficient: 0.05,
                depth_levels: 10,
                instantaneous_ratio: ratio,
            });
            // 验证模型可调用
            let _impact = m.compute_impact(Quantity::from_f64(1.0), Side::Buy, &sample_ob());
        }
    }

    /// 工厂传入 zero depth_levels
    #[test]
    fn test_factory_linear_zero_depth() {
        let m: Box<dyn ImpactModel> = create_model(ImpactModelConfig::Linear {
            coefficient: 0.05,
            depth_levels: 0,
            instantaneous_ratio: 0.7,
        });
        // depth=0 ⇒ take(0) ⇒ 总深度 = 0 ⇒ 零冲击
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert_eq!(impact, Impact::zero());
    }

    /// 工厂 PowerLaw exponent 越界
    #[test]
    #[should_panic(expected = "幂律指数应在 (0, 2] 范围")]
    fn test_factory_power_law_exponent_too_large_panics() {
        let _: Box<dyn ImpactModel> = create_model(ImpactModelConfig::PowerLaw {
            coefficient: 0.1,
            exponent: 2.5,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
        });
    }

    /// 工厂 PowerLaw exponent = 2 边界
    #[test]
    fn test_factory_power_law_exponent_two_ok() {
        let m: Box<dyn ImpactModel> = create_model(ImpactModelConfig::PowerLaw {
            coefficient: 0.01,
            exponent: 2.0,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
        });
        let _impact = m.compute_impact(Quantity::from_f64(1.0), Side::Buy, &sample_ob());
    }

    /// ImpactModel 应满足 Send + Sync（可跨线程使用）
    #[test]
    fn test_impact_model_trait_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        // 三种模型都是 Send + Sync
        assert_send_sync::<LinearImpactModel>();
        assert_send_sync::<PowerLawImpactModel>();
        // AdaptiveImpactModel 持有 Box<dyn ImpactModel>，也应为 Send + Sync
        assert_send_sync::<AdaptiveImpactModel>();
    }
}
