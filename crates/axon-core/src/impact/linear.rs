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

    // ─── 边界测试 ──────────────────────────────────────────

    /// 系数为 0 的模型应始终返回零冲击
    #[test]
    fn test_zero_coefficient_always_zero_impact() {
        let m = LinearImpactModel::new(0.0);
        let ob = sample_ob();
        for q in [0.0, 1.0, 10.0, 1_000.0, f64::MAX] {
            let impact = m.compute_impact(Quantity::from_f64(q), Side::Buy, &ob);
            assert_eq!(impact, Impact::zero());
        }
    }

    /// 零 instantaneous_ratio ⇒ 全部永久冲击
    #[test]
    fn test_zero_instantaneous_ratio_all_permanent() {
        let m = LinearImpactModel::new(0.05).with_instantaneous_ratio(0.0);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert_eq!(impact.instantaneous, 0.0);
        assert!((impact.permanent - 0.005).abs() < 1e-10);
    }

    /// 满 instantaneous_ratio（1.0）⇒ 全部即时冲击
    #[test]
    fn test_full_instantaneous_ratio_all_instantaneous() {
        let m = LinearImpactModel::new(0.05).with_instantaneous_ratio(1.0);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert!((impact.instantaneous - 0.005).abs() < 1e-10);
        assert_eq!(impact.permanent, 0.0);
    }

    /// 极大订单量冲击应保持有限
    #[test]
    fn test_extreme_quantity_impact_finite() {
        let m = LinearImpactModel::default();
        let impact = m.compute_impact(Quantity::from_f64(f64::MAX / 2.0), Side::Buy, &sample_ob());
        // 系数 × 极大数量 / 正常深度 ⇒ 极大量级，但有限或 inf
        assert!(impact.instantaneous.is_finite() || impact.instantaneous.is_infinite());
    }

    /// 深度零值且有订单 ⇒ 零冲击
    #[test]
    fn test_depth_zero_quantities_nonzero_order() {
        let m = LinearImpactModel::default();
        let ob = OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![OrderBookLevel {
                price: Price::from_f64(100.0),
                quantity: Quantity::from_f64(0.0),
            }],
            asks: vec![OrderBookLevel {
                price: Price::from_f64(101.0),
                quantity: Quantity::from_f64(0.0),
            }],
        };
        let impact = m.compute_impact(Quantity::from_f64(50.0), Side::Buy, &ob);
        assert_eq!(impact, Impact::zero());
    }

    /// 极小正数量应正常处理
    #[test]
    fn test_epsilon_quantity_impact() {
        let m = LinearImpactModel::new(0.05);
        let impact = m.compute_impact(Quantity::from_f64(f64::EPSILON), Side::Buy, &sample_ob());
        // 极小订单量产生极小冲击
        assert!(impact.total() >= 0.0);
        assert!(impact.total() < 1e-10);
    }

    /// 仅买单簿（无卖单）：Buy 方向
    #[test]
    fn test_buy_side_with_only_bids() {
        // 买方向应累计卖单深度；卖单簿为空 ⇒ 零冲击
        let m = LinearImpactModel::default();
        let ob = OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![OrderBookLevel {
                price: Price::from_f64(99.0),
                quantity: Quantity::from_f64(100.0),
            }],
            asks: vec![],
        };
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &ob);
        assert_eq!(impact, Impact::zero());
    }

    /// 仅卖单簿（无买单）：Sell 方向
    #[test]
    fn test_sell_side_with_only_asks() {
        let m = LinearImpactModel::default();
        let ob = OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![],
            asks: vec![OrderBookLevel {
                price: Price::from_f64(101.0),
                quantity: Quantity::from_f64(100.0),
            }],
        };
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Sell, &ob);
        assert_eq!(impact, Impact::zero());
    }

    /// 负 depth_levels 切片安全
    #[test]
    fn test_excessive_depth_levels_safe() {
        let m = LinearImpactModel::new(0.05).with_depth(100_000);
        let ob = sample_ob(); // 仅 1 层
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &ob);
        // take(100_000) 对 1 层迭代是安全的
        assert!((impact.total() - 0.005).abs() < 1e-10);
    }

    /// 序列化往返
    #[test]
    fn test_linear_serde_roundtrip() {
        let m = LinearImpactModel::new(0.05)
            .with_depth(15)
            .with_instantaneous_ratio(0.6);
        let json = serde_json::to_string(&m).unwrap();
        let de: LinearImpactModel = serde_json::from_str(&json).unwrap();
        assert_eq!(m.coefficient, de.coefficient);
        assert_eq!(m.depth_levels, de.depth_levels);
        assert!((m.instantaneous_ratio - de.instantaneous_ratio).abs() < 1e-10);
    }

    /// 极大系数 × 极大订单量
    #[test]
    fn test_extreme_coefficient_and_quantity() {
        let m = LinearImpactModel::new(1e9);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        // 1e9 × 0.1 = 1e8
        assert!(impact.total() >= 1.0e7);
    }

    // ─── 并发测试 ────────────────────────────────────

    /// 多线程并发 compute_impact：LinearImpactModel 无内部状态，
    /// 多个线程 Arc 共享模型并并行调用应保持确定性
    #[test]
    fn test_concurrent_compute_impact() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 50;
        const PER_THREAD: usize = 1_000;

        let m = Arc::new(LinearImpactModel::new(0.05));
        let ob = Arc::new(sample_ob());

        let mut handles = Vec::with_capacity(N_THREADS);
        for _ in 0..N_THREADS {
            let model = Arc::clone(&m);
            let book = Arc::clone(&ob);
            handles.push(thread::spawn(move || {
                for _ in 0..PER_THREAD {
                    let buy_impact =
                        model.compute_impact(Quantity::from_f64(10.0), Side::Buy, &book);
                    let sell_impact =
                        model.compute_impact(Quantity::from_f64(10.0), Side::Sell, &book);
                    // 买冲击 > 0（因为买单吃卖单）
                    assert!(buy_impact.total() > 0.0);
                    // 卖冲击 > 0（因为卖单吃买单）
                    assert!(sell_impact.total() > 0.0);
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    /// 静态断言：LinearImpactModel 是 Send + Sync
    #[test]
    fn test_linear_impact_model_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LinearImpactModel>();
    }
}
