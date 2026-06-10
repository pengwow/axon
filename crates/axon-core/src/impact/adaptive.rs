//! 自适应冲击模型
//!
//! 在基础模型上叠加波动率缩放因子，使冲击在波动加剧时放大、波动平缓时缩小。
//!
//! 注：`AdaptiveImpactModel` 不实现 `Clone` / `PartialEq` / `Serialize` / `Deserialize`，
//! 因为 `Box<dyn ImpactModel>` 不支持这些 trait。如需序列化，应使用 `ImpactModelConfig`
//! + `create_model` 工厂路径。后续可引入 `typetag` crate 支持 trait object 序列化。

use super::traits::ImpactModel;
use super::types::Impact;
use crate::market::{OrderBookSnapshot, Side};
use crate::types::Quantity;

/// 自适应冲击模型
///
/// 在 `base_model` 输出基础上乘以 `volatility_scale × (1 + current_volatility)` 缩放因子。
pub struct AdaptiveImpactModel {
    /// 基础模型
    pub base_model: Box<dyn ImpactModel>,
    /// 波动率缩放因子（> 1 放大，< 1 缩小）
    pub volatility_scale: f64,
    /// 当前相对波动率（0.0 = 无波动，1.0 = 100% 历史均值波动）
    pub current_volatility: f64,
}

impl std::fmt::Debug for AdaptiveImpactModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdaptiveImpactModel")
            .field("base_model", &self.base_model.name())
            .field("volatility_scale", &self.volatility_scale)
            .field("current_volatility", &self.current_volatility)
            .finish()
    }
}

impl AdaptiveImpactModel {
    /// 创建新自适应模型
    pub fn new(base_model: Box<dyn ImpactModel>, volatility_scale: f64) -> Self {
        assert!(volatility_scale >= 0.0, "波动率缩放因子必须非负");
        Self {
            base_model,
            volatility_scale,
            current_volatility: 0.0,
        }
    }

    /// 设置当前相对波动率
    pub fn with_volatility(mut self, vol: f64) -> Self {
        assert!(vol >= 0.0, "波动率必须非负");
        self.current_volatility = vol;
        self
    }
}

impl ImpactModel for AdaptiveImpactModel {
    fn compute_impact(
        &self,
        order_quantity: Quantity,
        side: Side,
        order_book: &OrderBookSnapshot,
    ) -> Impact {
        let base = self
            .base_model
            .compute_impact(order_quantity, side, order_book);

        // 缩放因子 = volatility_scale × (1 + current_volatility)
        let scale = self.volatility_scale * (1.0 + self.current_volatility);

        Impact {
            instantaneous: base.instantaneous * scale,
            permanent: base.permanent * scale,
        }
    }

    fn name(&self) -> &str {
        "AdaptiveImpact"
    }

    fn params(&self) -> String {
        format!(
            "base={}, vol_scale={}, cur_vol={}",
            self.base_model.name(),
            self.volatility_scale,
            self.current_volatility
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::impact::{Impact, ImpactModel};
    use crate::market::{OrderBookLevel, OrderBookSnapshot};
    use crate::time::Timestamp;
    use crate::types::{Price, Quantity};

    fn sample_ob() -> OrderBookSnapshot {
        OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![],
            asks: vec![OrderBookLevel {
                price: Price::from_f64(100.0),
                quantity: Quantity::from_f64(100.0),
            }],
        }
    }

    #[test]
    #[should_panic(expected = "波动率缩放因子必须非负")]
    fn test_new_rejects_negative_volatility_scale() {
        let base: Box<dyn ImpactModel> = Box::new(crate::impact::LinearImpactModel::new(0.05));
        AdaptiveImpactModel::new(base, -0.1);
    }

    #[test]
    fn test_with_volatility() {
        let base: Box<dyn ImpactModel> = Box::new(crate::impact::LinearImpactModel::new(0.05));
        let m = AdaptiveImpactModel::new(base, 1.0).with_volatility(0.5);
        assert!((m.current_volatility - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_compute_impact_with_zero_volatility_equals_base() {
        // volatility_scale=1.0, current_volatility=0.0 ⇒ 缩放因子=1.0 ⇒ 等价于基础模型
        let base: Box<dyn ImpactModel> = Box::new(crate::impact::LinearImpactModel::new(0.05));
        let base_impact = base.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());

        // 重建一个独立的 base 模型以供 adaptive 使用
        let base2: Box<dyn ImpactModel> = Box::new(crate::impact::LinearImpactModel::new(0.05));
        let adaptive = AdaptiveImpactModel::new(base2, 1.0);
        let adaptive_impact =
            adaptive.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert!((adaptive_impact.total() - base_impact.total()).abs() < 1e-10);
    }

    #[test]
    fn test_compute_impact_with_volatility_scales() {
        // volatility_scale=2.0, current_volatility=0.5 ⇒ 缩放因子=2.0 × 1.5 = 3.0
        let base: Box<dyn ImpactModel> = Box::new(crate::impact::LinearImpactModel::new(0.05));
        let adaptive = AdaptiveImpactModel::new(base, 2.0).with_volatility(0.5);
        let impact = adaptive.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        // 基础冲击 0.005，缩放 3.0 ⇒ 0.015
        assert!((impact.total() - 0.015).abs() < 1e-10);
    }

    #[test]
    fn test_compute_impact_zero_volatility_scale_zeroes_out() {
        // volatility_scale=0.0 ⇒ 缩放因子=0.0 ⇒ 零冲击
        let base: Box<dyn ImpactModel> = Box::new(crate::impact::LinearImpactModel::new(0.05));
        let adaptive = AdaptiveImpactModel::new(base, 0.0);
        let impact = adaptive.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert_eq!(impact, Impact::zero());
    }

    #[test]
    fn test_name_and_params() {
        let base: Box<dyn ImpactModel> = Box::new(crate::impact::LinearImpactModel::new(0.05));
        let m = AdaptiveImpactModel::new(base, 1.5);
        assert_eq!(m.name(), "AdaptiveImpact");
        let p = m.params();
        assert!(p.contains("LinearImpact"));
        assert!(p.contains("vol_scale=1.5"));
    }
}
