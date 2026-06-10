//! 线性冲击模型
//!
//! `impact = coefficient × (order_quantity / total_depth)`
//!
//! 适用于流动性好、订单量较小的场景。

use serde::{Deserialize, Serialize};

use super::traits::ImpactModel;
use super::types::Impact;
use crate::market::{OrderBookSnapshot, Side};
use crate::types::Quantity;

/// 线性冲击模型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearImpactModel {
    /// 冲击系数（通常 0.01-0.1）
    pub coefficient: f64,
    /// 使用的深度层级数
    pub depth_levels: usize,
    /// 即时/永久冲击比例（0.0-1.0，1.0 = 全部即时）
    pub instantaneous_ratio: f64,
}

impl LinearImpactModel {
    /// 创建新模型（默认 depth = 10，inst_ratio = 0.7）
    pub fn new(coefficient: f64) -> Self {
        assert!(coefficient >= 0.0, "冲击系数必须非负");
        Self {
            coefficient,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
        }
    }

    /// 设置深度层级
    pub fn with_depth(mut self, levels: usize) -> Self {
        self.depth_levels = levels;
        self
    }

    /// 设置即时冲击比例
    pub fn with_instantaneous_ratio(mut self, ratio: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&ratio),
            "instantaneous_ratio 必须在 [0, 1] 范围"
        );
        self.instantaneous_ratio = ratio;
        self
    }
}

impl Default for LinearImpactModel {
    fn default() -> Self {
        Self::new(0.05)
    }
}

impl ImpactModel for LinearImpactModel {
    fn compute_impact(
        &self,
        order_quantity: Quantity,
        side: Side,
        order_book: &OrderBookSnapshot,
    ) -> Impact {
        if order_book.asks.is_empty() && order_book.bids.is_empty() {
            return Impact::zero();
        }

        // 累加指定方向前 depth_levels 层的总深度
        let total_depth: f64 = match side {
            Side::Buy => order_book
                .asks
                .iter()
                .take(self.depth_levels)
                .map(|l| l.quantity.as_f64())
                .sum(),
            Side::Sell => order_book
                .bids
                .iter()
                .take(self.depth_levels)
                .map(|l| l.quantity.as_f64())
                .sum(),
        };

        if total_depth <= 0.0 {
            return Impact::zero();
        }

        let impact_magnitude = self.coefficient * (order_quantity.as_f64() / total_depth);
        let instantaneous = impact_magnitude * self.instantaneous_ratio;
        let permanent = impact_magnitude * (1.0 - self.instantaneous_ratio);

        Impact {
            instantaneous,
            permanent,
        }
    }

    fn name(&self) -> &str {
        "LinearImpact"
    }

    fn params(&self) -> String {
        format!(
            "coefficient={}, depth={}, inst_ratio={}",
            self.coefficient, self.depth_levels, self.instantaneous_ratio
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::{OrderBookLevel, OrderBookSnapshot};
    use crate::time::Timestamp;
    use crate::types::Price;

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
    #[should_panic(expected = "冲击系数必须非负")]
    fn test_new_rejects_negative_coefficient() {
        LinearImpactModel::new(-0.1);
    }

    #[test]
    fn test_new_uses_defaults() {
        let m = LinearImpactModel::new(0.05);
        assert_eq!(m.coefficient, 0.05);
        assert_eq!(m.depth_levels, 10);
        assert!((m.instantaneous_ratio - 0.7).abs() < 1e-10);
    }

    #[test]
    fn test_with_depth() {
        let m = LinearImpactModel::new(0.05).with_depth(5);
        assert_eq!(m.depth_levels, 5);
    }

    #[test]
    #[should_panic(expected = "必须在 [0, 1] 范围")]
    fn test_with_instantaneous_ratio_rejects_out_of_range() {
        LinearImpactModel::new(0.05).with_instantaneous_ratio(1.5);
    }

    #[test]
    fn test_default_model() {
        let m = LinearImpactModel::default();
        assert_eq!(m.coefficient, 0.05);
    }

    #[test]
    fn test_compute_impact_empty_orderbook() {
        let m = LinearImpactModel::default();
        let ob = OrderBookSnapshot::empty(Timestamp::from_nanos(0));
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &ob);
        assert_eq!(impact, Impact::zero());
    }

    #[test]
    fn test_compute_impact_buy_proportional() {
        let m = LinearImpactModel::new(0.05);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        // 0.05 × (10 / 100) = 0.005 total
        assert!((impact.total() - 0.005).abs() < 1e-10);
        // 70% instantaneous, 30% permanent
        assert!((impact.instantaneous - 0.0035).abs() < 1e-10);
        assert!((impact.permanent - 0.0015).abs() < 1e-10);
    }

    #[test]
    fn test_compute_impact_sell() {
        let m = LinearImpactModel::new(0.05);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Sell, &sample_ob());
        // 卖单冲击累计 bids
        assert!((impact.total() - 0.005).abs() < 1e-10);
    }

    #[test]
    fn test_compute_impact_zero_quantity() {
        let m = LinearImpactModel::new(0.05);
        let impact = m.compute_impact(Quantity::from_f64(0.0), Side::Buy, &sample_ob());
        assert_eq!(impact, Impact::zero());
    }

    #[test]
    fn test_compute_impact_zero_depth() {
        let m = LinearImpactModel::new(0.05);
        let ob = OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![OrderBookLevel {
                price: Price::from_f64(99.0),
                quantity: Quantity::from_f64(0.0),
            }],
            asks: vec![OrderBookLevel {
                price: Price::from_f64(100.0),
                quantity: Quantity::from_f64(0.0),
            }],
        };
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &ob);
        assert_eq!(impact, Impact::zero());
    }

    #[test]
    fn test_compute_impact_very_large_quantity() {
        let m = LinearImpactModel::new(0.05);
        let impact = m.compute_impact(Quantity::from_f64(10_000.0), Side::Buy, &sample_ob());
        // 0.05 × (10_000 / 100) = 5.0
        assert!((impact.total() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_depth_levels_limits_traversal() {
        let m = LinearImpactModel::new(0.05).with_depth(1);
        // 仅取第 1 层（asks[0] = 100），第二层（200）不参与
        let ob = OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![],
            asks: vec![
                OrderBookLevel {
                    price: Price::from_f64(100.0),
                    quantity: Quantity::from_f64(50.0),
                },
                OrderBookLevel {
                    price: Price::from_f64(101.0),
                    quantity: Quantity::from_f64(1000.0),
                },
            ],
        };
        let impact = m.compute_impact(Quantity::from_f64(5.0), Side::Buy, &ob);
        // 0.05 × (5 / 50) = 0.005
        assert!((impact.total() - 0.005).abs() < 1e-10);
    }

    #[test]
    fn test_name_and_params() {
        let m = LinearImpactModel::new(0.05).with_depth(5);
        assert_eq!(m.name(), "LinearImpact");
        let p = m.params();
        assert!(p.contains("0.05"));
        assert!(p.contains("depth=5"));
    }
}
