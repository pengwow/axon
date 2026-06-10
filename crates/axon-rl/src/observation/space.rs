//! 默认观测空间实现
//!
//! 组合特征提取 + 归一化 + 窗口聚合，对外暴露 `ObservationSpace` trait。

use crate::observation::error::ObservationError;
use crate::observation::normalizer::RunningStats;
use crate::observation::types::{
    BoxSpace, DType, FeatureConfig, MarketState, Observation, ObservationSpace,
    extract_feature_value, normalize_and_clip,
};

/// 默认观测空间：特征工程管线
///
/// 内部维护环形缓冲区 + 每个特征的运行时统计量。
pub struct DefaultObservationSpace {
    /// 窗口大小（保留最近 N 个 tick）
    pub window_size: usize,
    /// 特征配置列表
    pub features: Vec<FeatureConfig>,
    /// 每个特征的运行时统计量
    pub(crate) running_stats: Vec<RunningStats>,
}

impl DefaultObservationSpace {
    /// 构造默认观测空间（Z-Score 归一化）
    pub fn new(window_size: usize, features: Vec<FeatureConfig>) -> Result<Self, ObservationError> {
        crate::observation::validate_observation_space(&features, window_size)?;
        let n = features.len();
        Ok(Self {
            window_size,
            features,
            running_stats: vec![RunningStats::new(); n],
        })
    }

    /// 获取某个特征的运行时统计量（仅测试可见）
    #[cfg(test)]
    pub fn running_stats(&self, idx: usize) -> &RunningStats {
        &self.running_stats[idx]
    }

    /// 重置所有运行时统计量
    pub fn reset_stats(&mut self) {
        for stats in &mut self.running_stats {
            stats.reset();
        }
    }
}

impl ObservationSpace for DefaultObservationSpace {
    fn shape(&self) -> Vec<usize> {
        vec![self.features.len() * self.window_size]
    }

    fn low(&self) -> Vec<f64> {
        vec![-5.0; self.features.len() * self.window_size]
    }

    fn high(&self) -> Vec<f64> {
        vec![5.0; self.features.len() * self.window_size]
    }

    fn gymnasium_box(&self) -> BoxSpace {
        BoxSpace {
            shape: self.shape(),
            low: self.low(),
            high: self.high(),
            dtype: DType::Float32,
        }
    }

    fn build(
        &self,
        state: &MarketState,
        history: &[MarketState],
    ) -> Result<Observation, ObservationError> {
        let num_features = self.features.len();
        let window = self.window_size;
        let mut features = Vec::with_capacity(num_features * window);
        let mut feature_names = Vec::with_capacity(num_features * window);

        // 起始索引：保留最多 window 个历史点
        let start = if history.len() >= window {
            history.len() - window
        } else {
            0
        };

        for (feat_idx, feat_config) in self.features.iter().enumerate() {
            for t in start..history.len() {
                let raw = extract_feature_value(&feat_config.source, &history[t], &history[..t])?;
                let normalized = normalize_and_clip(
                    raw,
                    &feat_config.normalizer,
                    &self.running_stats[feat_idx],
                    feat_config.clip_range,
                );
                features.push(normalized);
                feature_names.push(format!("{}_t{}", feat_config.name, history.len() - t - 1));
            }
        }

        Ok(Observation {
            features,
            feature_names,
            timestamp: Some(state.timestamp),
        })
    }

    fn feature_names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(self.features.len() * self.window_size);
        for feat in &self.features {
            for t in 0..self.window_size {
                names.push(format!("{}_t{}", feat.name, t));
            }
        }
        names
    }

    fn num_features(&self) -> usize {
        self.features.len() * self.window_size
    }
}
