//! 归一化器与运行时统计量
//!
//! 实现 4 种归一化策略：Z-Score / Min-Max / Robust / Noop。
//! 配合 Welford 在线算法维护 O(1) 的运行时统计量。

use serde::{Deserialize, Serialize};

use crate::observation::types::NormalizerType;

// ── RunningStats (Welford 在线统计) ───────────────────────

/// 运行时归一化统计量
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunningStats {
    /// 当前均值
    pub mean: f64,
    /// Welford 算法的 M2 累加量
    pub m2: f64,
    /// 历史最小值
    pub min: f64,
    /// 历史最大值
    pub max: f64,
    /// 样本数
    pub count: u64,
    /// 中位数缓冲区（用于 Robust 归一化）
    pub median_buffer: Vec<f64>,
}

impl Default for RunningStats {
    fn default() -> Self {
        Self::new()
    }
}

impl RunningStats {
    /// 构造空统计量
    pub fn new() -> Self {
        Self {
            mean: 0.0,
            m2: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            count: 0,
            median_buffer: Vec::with_capacity(1024),
        }
    }

    /// Welford 在线更新 O(1)
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        if self.median_buffer.len() < self.median_buffer.capacity() {
            self.median_buffer.push(value);
        }
    }

    /// 当前方差
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    /// 当前标准差
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// 重置（清空所有统计量）
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ── Normalizer trait ─────────────────────────────────────

/// 归一化器 trait
pub trait Normalizer: Send + Sync {
    /// 归一化单个值
    fn normalize(&self, value: f64, stats: &RunningStats) -> f64;
    /// 更新运行时统计量
    fn update_stats(&self, stats: &mut RunningStats, value: f64);
    /// 重置运行时统计量
    fn reset(&self, stats: &mut RunningStats);
}

// ── Z-Score 归一化 ───────────────────────────────────────

/// Z-Score 归一化：(x - mean) / std，clip 到 ±5σ
pub struct ZScoreNormalizer;

impl Normalizer for ZScoreNormalizer {
    fn normalize(&self, value: f64, stats: &RunningStats) -> f64 {
        if stats.count < 2 {
            return 0.0;
        }
        let std = stats.std_dev();
        if std < 1e-8 {
            return 0.0;
        }
        let z = (value - stats.mean) / std;
        z.clamp(-5.0, 5.0)
    }

    fn update_stats(&self, stats: &mut RunningStats, value: f64) {
        stats.update(value);
    }

    fn reset(&self, stats: &mut RunningStats) {
        stats.reset();
    }
}

// ── Min-Max 归一化 ───────────────────────────────────────

/// Min-Max 归一化：(x - min) / (max - min) → [0, 1]
pub struct MinMaxNormalizer;

impl Normalizer for MinMaxNormalizer {
    fn normalize(&self, value: f64, stats: &RunningStats) -> f64 {
        let range = stats.max - stats.min;
        if range < 1e-8 {
            return 0.5;
        }
        let normalized = (value - stats.min) / range;
        normalized.clamp(0.0, 1.0)
    }

    fn update_stats(&self, stats: &mut RunningStats, value: f64) {
        stats.update(value);
    }

    fn reset(&self, stats: &mut RunningStats) {
        stats.reset();
    }
}

// ── Robust 归一化（中位数 + IQR）────────────────────────

/// Robust 归一化：(x - median) / IQR，抗异常值
pub struct RobustNormalizer;

impl Normalizer for RobustNormalizer {
    fn normalize(&self, value: f64, stats: &RunningStats) -> f64 {
        if stats.median_buffer.is_empty() {
            return 0.0;
        }
        let mut sorted = stats.median_buffer.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = sorted.len();
        let median = if n.is_multiple_of(2) {
            (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
        } else {
            sorted[n / 2]
        };
        let q1 = sorted[n / 4];
        let q3 = sorted[3 * n / 4];
        let iqr = q3 - q1;
        if iqr < 1e-8 {
            return 0.0;
        }
        let z = (value - median) / iqr;
        z.clamp(-5.0, 5.0)
    }

    fn update_stats(&self, stats: &mut RunningStats, value: f64) {
        stats.update(value);
    }

    fn reset(&self, stats: &mut RunningStats) {
        stats.reset();
    }
}

// ── Noop 归一化 ──────────────────────────────────────────

/// 不做归一化（恒等映射）
pub struct NoopNormalizer;

impl Normalizer for NoopNormalizer {
    fn normalize(&self, value: f64, _stats: &RunningStats) -> f64 {
        value
    }

    fn update_stats(&self, stats: &mut RunningStats, value: f64) {
        stats.update(value);
    }

    fn reset(&self, stats: &mut RunningStats) {
        stats.reset();
    }
}

// ── 工厂函数 ─────────────────────────────────────────────

/// 根据 `NormalizerType` 构造对应归一化器
pub fn make_normalizer(nt: &NormalizerType) -> Box<dyn Normalizer> {
    match nt {
        NormalizerType::ZScore => Box::new(ZScoreNormalizer),
        NormalizerType::MinMax => Box::new(MinMaxNormalizer),
        NormalizerType::Robust => Box::new(RobustNormalizer),
        NormalizerType::None => Box::new(NoopNormalizer),
    }
}
