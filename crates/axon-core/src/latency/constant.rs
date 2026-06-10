//! 固定延迟模型
//!
//! 各路径使用预设的恒定延迟，适用于理想化基线回测或单元测试。

use std::collections::HashMap;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::traits::{LatencyModel, LatencyParams, PathType};

/// 固定延迟模型
///
/// 通过 `HashMap<PathType, Duration>` 存储各路径的固定延迟。
/// 未配置的路径采样时返回 `Duration::ZERO`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub struct ConstantLatencyModel {
    /// 各路径类型的固定延迟
    pub delays: HashMap<PathType, Duration>,
}

impl ConstantLatencyModel {
    /// 创建统一延迟模型（所有路径使用同一延迟）
    pub fn uniform(delay: Duration) -> Self {
        let mut delays = HashMap::new();
        for path in PathType::ALL {
            delays.insert(path, delay);
        }
        Self { delays }
    }

    /// 分别设置常见路径的延迟
    ///
    /// - `market_data`：行情延迟
    /// - `order_submit`：下单延迟
    /// - `order_cancel`：撤单延迟
    /// - `account_query`：复用下单延迟
    /// - `heartbeat`：默认 50ms
    pub fn with_paths(
        market_data: Duration,
        order_submit: Duration,
        order_cancel: Duration,
    ) -> Self {
        let mut delays = HashMap::new();
        delays.insert(PathType::MarketData, market_data);
        delays.insert(PathType::OrderSubmit, order_submit);
        delays.insert(PathType::OrderCancel, order_cancel);
        delays.insert(PathType::AccountQuery, order_submit);
        delays.insert(PathType::Heartbeat, Duration::from_millis(50));
        Self { delays }
    }

    /// 为单个路径设置延迟
    pub fn set_path(&mut self, path: PathType, delay: Duration) {
        self.delays.insert(path, delay);
    }

    /// 获取指定路径的延迟（若未配置返回 None）
    pub fn get(&self, path: PathType) -> Option<Duration> {
        self.delays.get(&path).copied()
    }
}

impl LatencyModel for ConstantLatencyModel {
    fn sample_delay(&self, path: PathType) -> Duration {
        self.delays.get(&path).copied().unwrap_or(Duration::ZERO)
    }

    fn name(&self) -> &str {
        "constant"
    }

    fn params(&self) -> LatencyParams {
        let count = self.delays.len() as f64;
        let sum_ms: f64 = self
            .delays
            .values()
            .map(|d| d.as_secs_f64() * 1000.0)
            .sum();
        let avg = if count > 0.0 { sum_ms / count } else { 0.0 };
        LatencyParams {
            model_type: "constant".to_string(),
            base_delay_ms: avg,
            jitter_ms: None,
            path_overrides: self
                .delays
                .iter()
                .map(|(k, v)| (*k, v.as_secs_f64() * 1000.0))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_constant_delay() {
        let model = ConstantLatencyModel::uniform(Duration::from_millis(5));
        for path in PathType::ALL {
            assert_eq!(model.sample_delay(path), Duration::from_millis(5));
        }
    }

    #[test]
    fn test_with_paths_uses_individual_delays() {
        let model = ConstantLatencyModel::with_paths(
            Duration::from_millis(2),
            Duration::from_millis(10),
            Duration::from_millis(8),
        );
        assert_eq!(
            model.sample_delay(PathType::MarketData),
            Duration::from_millis(2)
        );
        assert_eq!(
            model.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(10)
        );
        assert_eq!(
            model.sample_delay(PathType::OrderCancel),
            Duration::from_millis(8)
        );
        // AccountQuery 复用 order_submit
        assert_eq!(
            model.sample_delay(PathType::AccountQuery),
            Duration::from_millis(10)
        );
        // Heartbeat 默认 50ms
        assert_eq!(
            model.sample_delay(PathType::Heartbeat),
            Duration::from_millis(50)
        );
    }

    #[test]
    fn test_unconfigured_path_returns_zero() {
        let mut model = ConstantLatencyModel::uniform(Duration::from_millis(1));
        model.delays.remove(&PathType::Heartbeat);
        assert_eq!(model.sample_delay(PathType::Heartbeat), Duration::ZERO);
    }

    #[test]
    fn test_set_path_overrides() {
        let mut model = ConstantLatencyModel::uniform(Duration::from_millis(1));
        model.set_path(PathType::OrderSubmit, Duration::from_millis(20));
        assert_eq!(
            model.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(20)
        );
    }

    #[test]
    fn test_name_and_params() {
        let model = ConstantLatencyModel::uniform(Duration::from_millis(5));
        assert_eq!(model.name(), "constant");
        let p = model.params();
        assert_eq!(p.model_type, "constant");
        assert!((p.base_delay_ms - 5.0).abs() < 1e-9);
        assert!(p.jitter_ms.is_none());
        assert_eq!(p.path_overrides.len(), PathType::ALL.len());
    }
}
