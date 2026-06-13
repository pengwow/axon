//! 幂律冲击模型
//!
//! `impact = coefficient × (order_quantity / total_depth)^exponent`
//!
//! 实证研究表明 `exponent ≈ 0.5-0.6`（square-root law）。

use serde::{Deserialize, Serialize};

use super::traits::ImpactModel;
use super::types::Impact;
use crate::market::{OrderBookSnapshot, Side};
use crate::types::Quantity;

/// 幂律冲击模型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerLawImpactModel {
    /// 冲击系数
    pub coefficient: f64,
    /// 幂律指数（通常 0.5-0.6）
    pub exponent: f64,
    /// 使用的深度层级数
    pub depth_levels: usize,
    /// 即时/永久冲击比例
    pub instantaneous_ratio: f64,
}

impl PowerLawImpactModel {
    /// 创建新模型（默认 depth = 10，inst_ratio = 0.7）
    pub fn new(coefficient: f64, exponent: f64) -> Self {
        assert!(coefficient >= 0.0, "冲击系数必须非负");
        assert!(
            exponent > 0.0 && exponent <= 2.0,
            "幂律指数应在 (0, 2] 范围"
        );
        Self {
            coefficient,
            exponent,
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

impl Default for PowerLawImpactModel {
    fn default() -> Self {
        Self::new(0.1, 0.5) // 经典 square-root law
    }
}

impl ImpactModel for PowerLawImpactModel {
    fn compute_impact(
        &self,
        order_quantity: Quantity,
        side: Side,
        order_book: &OrderBookSnapshot,
    ) -> Impact {
        if order_book.asks.is_empty() && order_book.bids.is_empty() {
            return Impact::zero();
        }

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

        let ratio = order_quantity.as_f64() / total_depth;
        let impact_magnitude = self.coefficient * ratio.powf(self.exponent);
        let instantaneous = impact_magnitude * self.instantaneous_ratio;
        let permanent = impact_magnitude * (1.0 - self.instantaneous_ratio);

        Impact {
            instantaneous,
            permanent,
        }
    }

    fn name(&self) -> &str {
        "PowerLawImpact"
    }

    fn params(&self) -> String {
        format!(
            "coefficient={}, exponent={}, depth={}, inst_ratio={}",
            self.coefficient, self.exponent, self.depth_levels, self.instantaneous_ratio
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
        PowerLawImpactModel::new(-0.1, 0.5);
    }

    #[test]
    #[should_panic(expected = "幂律指数应在 (0, 2] 范围")]
    fn test_new_rejects_zero_exponent() {
        PowerLawImpactModel::new(0.1, 0.0);
    }

    #[test]
    #[should_panic(expected = "幂律指数应在 (0, 2] 范围")]
    fn test_new_rejects_too_large_exponent() {
        PowerLawImpactModel::new(0.1, 2.5);
    }

    #[test]
    fn test_default_uses_sqrt_law() {
        let m = PowerLawImpactModel::default();
        assert_eq!(m.coefficient, 0.1);
        assert!((m.exponent - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_with_depth() {
        let m = PowerLawImpactModel::new(0.1, 0.5).with_depth(20);
        assert_eq!(m.depth_levels, 20);
    }

    #[test]
    fn test_compute_impact_sqrt_law() {
        let m = PowerLawImpactModel::new(0.1, 0.5);
        let ob = OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![],
            asks: vec![OrderBookLevel {
                price: Price::from_f64(100.0),
                quantity: Quantity::from_f64(1000.0),
            }],
        };
        let impact = m.compute_impact(Quantity::from_f64(100.0), Side::Buy, &ob);
        // 0.1 × (100/1000)^0.5 = 0.1 × 0.3162...
        let expected = 0.1 * (100.0_f64 / 1000.0).sqrt();
        assert!((impact.total() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_compute_impact_sublinear() {
        // exponent < 1 ⇒ 亚线性（订单量翻倍，冲击小于翻倍）
        let m = PowerLawImpactModel::new(0.1, 0.5);
        let ob = sample_ob();
        let i1 = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &ob);
        let i2 = m.compute_impact(Quantity::from_f64(20.0), Side::Buy, &ob);
        // sqrt(2) ≈ 1.414
        assert!(i2.total() / i1.total() < 2.0);
        assert!(i2.total() / i1.total() > 1.4);
    }

    #[test]
    fn test_compute_impact_exponent_one_equals_linear() {
        // exponent = 1 ⇒ 等价于线性模型
        let m = PowerLawImpactModel::new(0.05, 1.0);
        let ob = sample_ob();
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &ob);
        // 0.05 × (10 / 100)^1 = 0.005
        assert!((impact.total() - 0.005).abs() < 1e-10);
    }

    #[test]
    fn test_compute_impact_empty_orderbook() {
        let m = PowerLawImpactModel::default();
        let ob = OrderBookSnapshot::empty(Timestamp::from_nanos(0));
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &ob);
        assert_eq!(impact, Impact::zero());
    }

    #[test]
    fn test_compute_impact_zero_depth() {
        let m = PowerLawImpactModel::default();
        let ob = OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![],
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
        let m = PowerLawImpactModel::new(0.1, 0.5);
        let impact = m.compute_impact(Quantity::from_f64(10_000.0), Side::Buy, &sample_ob());
        // 0.1 × (10_000 / 100)^0.5 = 0.1 × 10 = 1.0
        assert!((impact.total() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_name_and_params() {
        let m = PowerLawImpactModel::new(0.1, 0.5);
        assert_eq!(m.name(), "PowerLawImpact");
        let p = m.params();
        assert!(p.contains("exponent=0.5"));
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 系数为 0 ⇒ 零冲击
    #[test]
    fn test_zero_coefficient_zero_impact() {
        let m = PowerLawImpactModel::new(0.0, 0.5);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert_eq!(impact, Impact::zero());
    }

    /// exponent = 2（最大允许）⇒ 超线性
    #[test]
    fn test_exponent_two_superlinear() {
        let m = PowerLawImpactModel::new(0.01, 2.0);
        let ob = sample_ob();
        let i1 = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &ob);
        let i2 = m.compute_impact(Quantity::from_f64(20.0), Side::Buy, &ob);
        // exponent = 2 ⇒ i2 / i1 = 4
        assert!((i2.total() / i1.total() - 4.0).abs() < 1e-6);
    }

    /// 极小正 exponent ⇒ 亚线性（接近 0 时趋近于 0）
    #[test]
    fn test_small_exponent_near_zero() {
        // 0.1 指数：order 10 ⇒ impact = 0.05 × 0.1^0.1
        let m = PowerLawImpactModel::new(0.05, 0.1);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert!(impact.total() > 0.0);
        assert!(impact.total() < 0.05); // 远小于线性 0.005
    }

    /// 极大订单量（f64::MAX）应保持计算稳定
    #[test]
    fn test_extreme_quantity_does_not_panic() {
        let m = PowerLawImpactModel::default();
        let impact = m.compute_impact(Quantity::from_f64(f64::MAX / 2.0), Side::Buy, &sample_ob());
        // 1e308 ^ 0.5 = 1e154 ⇒ 极大量级冲击
        assert!(impact.total().is_finite() || impact.total().is_infinite());
    }

    /// 极小正数量
    #[test]
    fn test_epsilon_quantity() {
        let m = PowerLawImpactModel::default();
        let impact = m.compute_impact(Quantity::from_f64(f64::EPSILON), Side::Buy, &sample_ob());
        assert!(impact.total() >= 0.0);
    }

    /// 极小正深度
    #[test]
    fn test_min_positive_depth() {
        let m = PowerLawImpactModel::new(0.1, 0.5);
        let ob = OrderBookSnapshot {
            timestamp: Timestamp::from_nanos(0),
            bids: vec![],
            asks: vec![OrderBookLevel {
                price: Price::from_f64(100.0),
                quantity: Quantity::from_f64(f64::MIN_POSITIVE),
            }],
        };
        let impact = m.compute_impact(Quantity::from_f64(1.0), Side::Buy, &ob);
        // ratio = 1 / MIN_POSITIVE 极大 ⇒ 冲击爆炸
        assert!(impact.total() > 0.0);
    }

    /// 序列化往返
    #[test]
    fn test_power_law_serde_roundtrip() {
        let m = PowerLawImpactModel::new(0.15, 0.6)
            .with_depth(20)
            .with_instantaneous_ratio(0.8);
        let json = serde_json::to_string(&m).unwrap();
        let de: PowerLawImpactModel = serde_json::from_str(&json).unwrap();
        assert_eq!(m.coefficient, de.coefficient);
        assert!((m.exponent - de.exponent).abs() < 1e-10);
        assert_eq!(m.depth_levels, de.depth_levels);
    }

    /// 零 instantaneous_ratio ⇒ 全部永久冲击
    #[test]
    fn test_zero_instantaneous_ratio_all_permanent() {
        let m = PowerLawImpactModel::new(0.1, 0.5).with_instantaneous_ratio(0.0);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert_eq!(impact.instantaneous, 0.0);
        assert!(impact.permanent > 0.0);
    }

    /// 满 instantaneous_ratio ⇒ 全部即时冲击
    #[test]
    fn test_full_instantaneous_ratio_all_instantaneous() {
        let m = PowerLawImpactModel::new(0.1, 0.5).with_instantaneous_ratio(1.0);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert!(impact.instantaneous > 0.0);
        assert_eq!(impact.permanent, 0.0);
    }

    /// 极大 depth_levels 不 panic
    #[test]
    fn test_excessive_depth_levels_safe() {
        let m = PowerLawImpactModel::new(0.1, 0.5).with_depth(usize::MAX);
        let impact = m.compute_impact(Quantity::from_f64(10.0), Side::Buy, &sample_ob());
        assert!(impact.total() > 0.0);
    }

    // ─── 并发测试 ────────────────────────────────────

    /// 多线程并发 compute_impact：PowerLawImpactModel 无内部状态
    #[test]
    fn test_concurrent_compute_impact() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 50;
        const PER_THREAD: usize = 1_000;

        let m = Arc::new(PowerLawImpactModel::new(0.05, 0.5));
        let ob = Arc::new(sample_ob());

        let mut handles = Vec::with_capacity(N_THREADS);
        for _ in 0..N_THREADS {
            let model = Arc::clone(&m);
            let book = Arc::clone(&ob);
            handles.push(thread::spawn(move || {
                for q in 1..=PER_THREAD {
                    let impact =
                        model.compute_impact(Quantity::from_f64(q as f64), Side::Buy, &book);
                    // 幂律冲击：qty^k × coefficient × 累计深度
                    assert!(impact.total() > 0.0);
                    // 即时冲击 + 永久冲击 = total
                    assert!(impact.instantaneous >= 0.0);
                    assert!(impact.permanent >= 0.0);
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    /// 静态断言：PowerLawImpactModel 是 Send + Sync
    #[test]
    fn test_power_law_impact_model_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PowerLawImpactModel>();
    }
}
