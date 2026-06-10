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
    pub fn realistic_combo(
        market_data_ms: f64,
        order_mean_ms: f64,
    ) -> CompositeLatencyModel {
        CompositeLatencyModel::new(Box::new(Self::constant(market_data_ms)))
            .with_path(
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
        assert_eq!(m.sample_delay(PathType::MarketData), Duration::from_millis(5));
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
}
