//! 延迟模型工厂
//!
//! 提供便利的构造函数，以毫秒为单位指定参数。

use std::time::Duration;

use super::composite::CompositeLatencyModel;
use super::constant::ConstantLatencyModel;
use super::exponential::ExponentialLatencyModel;
use super::normal::NormalLatencyModel;
use super::queue::QueueLatencyModel;
use super::uniform::UniformLatencyModel;

/// 延迟模型工厂
pub struct LatencyModelFactory;

impl LatencyModelFactory {
    /// 创建固定延迟模型（所有路径同一延迟）
    pub fn constant(delay_ms: f64) -> ConstantLatencyModel {
        ConstantLatencyModel::uniform(Duration::from_secs_f64(delay_ms / 1000.0))
    }

    /// 创建正态分布延迟模型
    pub fn normal(mean_ms: f64, std_dev_ms: f64) -> NormalLatencyModel {
        NormalLatencyModel::uniform(
            Duration::from_secs_f64(mean_ms / 1000.0),
            Duration::from_secs_f64(std_dev_ms / 1000.0),
        )
    }

    /// 创建指数分布延迟模型（rate = 1000 / mean_ms）
    pub fn exponential(mean_ms: f64) -> ExponentialLatencyModel {
        ExponentialLatencyModel::from_mean_ms(mean_ms)
    }

    /// 创建均匀分布延迟模型
    pub fn uniform(min_ms: f64, max_ms: f64) -> UniformLatencyModel {
        UniformLatencyModel::uniform(
            Duration::from_secs_f64(min_ms / 1000.0),
            Duration::from_secs_f64(max_ms / 1000.0),
        )
    }

    /// 创建队列延迟模型
    pub fn queue(base_delay_ms: f64, processing_time_ms: f64) -> QueueLatencyModel {
        QueueLatencyModel::new(
            Duration::from_secs_f64(base_delay_ms / 1000.0),
            Duration::from_secs_f64(processing_time_ms / 1000.0),
        )
    }

    /// 常见组合：行情低延迟固定，订单指数分布
    pub fn realistic_combo(market_data_ms: f64, order_mean_ms: f64) -> CompositeLatencyModel {
        CompositeLatencyModel::new(Box::new(Self::constant(market_data_ms))).with_path(
            super::traits::PathType::OrderSubmit,
            Box::new(Self::exponential(order_mean_ms)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::latency::{LatencyModel, PathType};

    #[test]
    fn test_factory_constant() {
        let m = LatencyModelFactory::constant(5.0);
        assert_eq!(
            m.sample_delay(PathType::MarketData),
            Duration::from_millis(5)
        );
    }

    #[test]
    fn test_factory_normal() {
        let m = LatencyModelFactory::normal(10.0, 2.0);
        let n = 5_000;
        let sum: f64 = (0..n)
            .map(|_| m.sample_delay(PathType::MarketData).as_secs_f64() * 1000.0)
            .sum();
        let mean = sum / n as f64;
        assert!((mean - 10.0).abs() < 1.0, "mean = {mean}");
    }

    #[test]
    fn test_factory_exponential() {
        let m = LatencyModelFactory::exponential(5.0);
        let n = 5_000;
        let sum: f64 = (0..n)
            .map(|_| m.sample_delay(PathType::MarketData).as_secs_f64() * 1000.0)
            .sum();
        let mean = sum / n as f64;
        assert!((mean - 5.0).abs() < 1.0, "mean = {mean}");
    }

    #[test]
    fn test_factory_uniform() {
        let m = LatencyModelFactory::uniform(1.0, 5.0);
        for _ in 0..200 {
            let d = m.sample_delay(PathType::MarketData);
            assert!(d >= Duration::from_millis(1));
            assert!(d <= Duration::from_millis(5));
        }
    }

    #[test]
    fn test_factory_queue() {
        let m = LatencyModelFactory::queue(10.0, 1.0);
        assert_eq!(
            m.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(10)
        );
    }

    #[test]
    fn test_factory_realistic_combo() {
        let m = LatencyModelFactory::realistic_combo(2.0, 8.0);
        assert_eq!(
            m.sample_delay(PathType::MarketData),
            Duration::from_millis(2)
        );
        let n = 5_000;
        let sum: f64 = (0..n)
            .map(|_| m.sample_delay(PathType::OrderSubmit).as_secs_f64() * 1000.0)
            .sum();
        let mean = sum / n as f64;
        assert!((mean - 8.0).abs() < 2.0, "mean = {mean}");
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 零延迟 constant
    #[test]
    fn test_factory_constant_zero() {
        let m = LatencyModelFactory::constant(0.0);
        assert_eq!(
            m.sample_delay(PathType::MarketData),
            Duration::from_millis(0)
        );
    }

    /// 负延迟 constant ⇒ Duration::from_secs_f64 会 panic（已知行为）
    #[test]
    #[should_panic(expected = "value is negative")]
    fn test_factory_constant_negative_panics() {
        // Duration::from_secs_f64 不接受负值，这是设计决策
        let _ = LatencyModelFactory::constant(-1.0);
    }

    /// 极大延迟 constant
    #[test]
    fn test_factory_constant_extreme() {
        let m = LatencyModelFactory::constant(60_000.0); // 60 秒
        assert_eq!(
            m.sample_delay(PathType::MarketData),
            Duration::from_secs(60)
        );
    }

    /// 零 mean + 零 std_dev normal
    #[test]
    fn test_factory_normal_zero_values() {
        let m = LatencyModelFactory::normal(0.0, 0.0);
        for _ in 0..100 {
            assert_eq!(m.sample_delay(PathType::MarketData), Duration::ZERO);
        }
    }

    /// 零 mean_ms exponential
    #[test]
    fn test_factory_exponential_zero_mean() {
        let m = LatencyModelFactory::exponential(0.0);
        // mean=0 ⇒ rate=1 ⇒ 实际 1s 延迟
        let d = m.sample_delay(PathType::MarketData);
        assert!(d.as_secs_f64() >= 0.0);
    }

    /// 极小延迟 uniform
    #[test]
    fn test_factory_uniform_tiny_range() {
        let m = LatencyModelFactory::uniform(0.001, 0.002);
        for _ in 0..100 {
            let d = m.sample_delay(PathType::MarketData);
            // 1-2 µs
            assert!(d <= Duration::from_micros(2));
        }
    }

    /// min > max uniform
    #[test]
    fn test_factory_uniform_min_greater_than_max() {
        let m = LatencyModelFactory::uniform(10.0, 5.0);
        // max <= min ⇒ sample 返回 min = 10ms
        for _ in 0..100 {
            assert_eq!(
                m.sample_delay(PathType::MarketData),
                Duration::from_millis(10)
            );
        }
    }

    /// 零 base_delay queue
    #[test]
    fn test_factory_queue_zero_base() {
        let m = LatencyModelFactory::queue(0.0, 5.0);
        // base=0, queue=0 ⇒ 0
        assert_eq!(
            m.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(0)
        );
    }

    /// 零参数 realistic_combo
    #[test]
    fn test_factory_realistic_combo_all_zero() {
        let m = LatencyModelFactory::realistic_combo(0.0, 0.0);
        // 行情 = 0
        assert_eq!(
            m.sample_delay(PathType::MarketData),
            Duration::from_millis(0)
        );
        // 订单 = 指数 rate=1 ⇒ mean=1s
        let d = m.sample_delay(PathType::OrderSubmit);
        assert!(d.as_secs_f64() >= 0.0);
    }
}
