//! 特征管道(骨架)
//!
//! 后续可扩展为向量化(SIMD)、自适应归一化、滚动统计等。
//! 当前提供:
//! - [`FeaturePipeline`]:串联归一化 + 滑动窗口聚合
//! - [`Normalizer`]:归一化策略 trait
//! - [`ZScoreNormalizer`]:z-score 实现(默认)
//! - [`FeatureMatrix`]:输出矩阵(`Vec<f32>` 表示)

use crate::dataset::Dataset;

// ─── 归一化 trait ──────────────────────────────────────────

/// 归一化策略
pub trait Normalizer: Send + Sync {
    /// 训练阶段:从 dataset 学到归一化参数
    fn fit(&mut self, ds: &Dataset);

    /// 推理阶段:把 dataset 转为 FeatureMatrix
    fn transform(&self, ds: &Dataset) -> FeatureMatrix;
}

/// z-score 归一化:`(x - mean) / std`
#[derive(Debug, Clone, Default)]
pub struct ZScoreNormalizer {
    mean: f64,
    std: f64,
}

impl ZScoreNormalizer {
    /// 构造空归一化器(未训练)
    pub fn new() -> Self {
        Self {
            mean: 0.0,
            std: 1.0,
        }
    }

    /// 构造带参归一化器
    pub fn with_params(mean: f64, std: f64) -> Self {
        Self {
            mean,
            std: if std > 0.0 { std } else { 1.0 },
        }
    }

    /// 当前均值
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// 当前标准差
    pub fn std(&self) -> f64 {
        self.std
    }
}

impl Normalizer for ZScoreNormalizer {
    fn fit(&mut self, ds: &Dataset) {
        if ds.is_empty() {
            return;
        }
        // PR5:走 batch 列式 buffer 零拷贝读(避免 Tick 中间表示)
        let mut sum = 0.0_f64;
        let mut sq_sum = 0.0_f64;
        let mut n = 0usize;
        for batch in ds.iter_batches() {
            let px = batch
                .column(1)
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .expect("col 1 Float64Array (schema-validated)");
            for i in 0..batch.num_rows() {
                let v = px.value(i);
                sum += v;
                sq_sum += v * v;
                n += 1;
            }
        }
        let mean = sum / n as f64;
        let variance = (sq_sum / n as f64) - mean * mean;
        self.mean = mean;
        self.std = variance.sqrt();
    }

    fn transform(&self, ds: &Dataset) -> FeatureMatrix {
        let mut data = Vec::with_capacity(ds.len());
        // PR5:列式读 price
        for batch in ds.iter_batches() {
            let px = batch
                .column(1)
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .expect("col 1 Float64Array (schema-validated)");
            for i in 0..batch.num_rows() {
                let p = px.value(i);
                let normalized = (p - self.mean) / self.std;
                data.push(normalized as f32);
            }
        }
        FeatureMatrix {
            data,
            n_samples: ds.len(),
            n_features: 1,
        }
    }
}

// ─── Feature 矩阵 ──────────────────────────────────────────

/// 特征矩阵
#[derive(Debug, Clone, Default)]
pub struct FeatureMatrix {
    /// 数据(行优先)
    pub data: Vec<f32>,
    /// 样本数
    pub n_samples: usize,
    /// 特征数
    pub n_features: usize,
}

impl FeatureMatrix {
    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 取 (sample, feature) 元素
    pub fn get(&self, sample: usize, feature: usize) -> Option<f32> {
        if sample >= self.n_samples || feature >= self.n_features {
            return None;
        }
        self.data.get(sample * self.n_features + feature).copied()
    }
}

// ─── 管道 ──────────────────────────────────────────────────

/// 特征管道
pub struct FeaturePipeline {
    normalizer: Option<Box<dyn Normalizer>>,
    window: usize,
}

impl std::fmt::Debug for FeaturePipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeaturePipeline")
            .field(
                "normalizer",
                &self.normalizer.as_ref().map(|_| "<normalizer>"),
            )
            .field("window", &self.window)
            .finish()
    }
}

impl FeaturePipeline {
    /// 构造空管道
    pub fn new() -> Self {
        Self {
            normalizer: None,
            window: 0,
        }
    }

    /// 设置归一化器
    pub fn with_normalizer(mut self, norm: Box<dyn Normalizer>) -> Self {
        self.normalizer = Some(norm);
        self
    }

    /// 设置滑动窗口大小(0 = 不聚合)
    pub fn with_window(mut self, window: usize) -> Self {
        self.window = window;
        self
    }

    /// 训练 + 推理两阶段(简单管线)
    pub fn fit_transform(&mut self, ds: &Dataset) -> FeatureMatrix {
        if let Some(norm) = self.normalizer.as_mut() {
            norm.fit(ds);
        }
        self.transform(ds)
    }

    /// 单独推理(归一化器必须已 fit)
    pub fn transform(&self, ds: &Dataset) -> FeatureMatrix {
        if let Some(norm) = self.normalizer.as_ref() {
            norm.transform(ds)
        } else {
            // 无归一化器:返回原始 f32(单特征=price)— PR5 列式读
            let mut data: Vec<f32> = Vec::with_capacity(ds.len());
            for batch in ds.iter_batches() {
                let px = batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<arrow::array::Float64Array>()
                    .expect("col 1 Float64Array (schema-validated)");
                for i in 0..batch.num_rows() {
                    data.push(px.value(i) as f32);
                }
            }
            FeatureMatrix {
                data,
                n_samples: ds.len(),
                n_features: 1,
            }
        }
    }
}

impl Default for FeaturePipeline {
    /// 默认实现(等价于 [`FeaturePipeline::new`])
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DataRequest, Frequency};
    use axon_core::market::{Side, Tick};
    use axon_core::time::Timestamp;
    use axon_core::types::{Price, Quantity};
    use chrono::Utc;

    fn make_dataset(prices: &[f64]) -> Dataset {
        let rows: Vec<Tick> = prices
            .iter()
            .enumerate()
            .map(|(i, p)| {
                Tick::new(
                    Timestamp::from_nanos(i as i64 * 1_000_000_000),
                    Price::from_f64(*p),
                    Quantity::from(1.0),
                    Side::Buy,
                )
            })
            .collect();
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        // PR5:走 from_ticks 桥接入口
        Dataset::from_ticks(rows, "test".into(), req).expect("from_ticks")
    }

    #[test]
    fn zscore_fit_then_transform_yields_zero_mean_unit_std() {
        let ds = make_dataset(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let mut norm = ZScoreNormalizer::new();
        norm.fit(&ds);
        let matrix = norm.transform(&ds);
        assert_eq!(matrix.n_samples, 5);
        // 均值 = 3, std = sqrt(2) ≈ 1.414
        let sum: f32 = matrix.data.iter().sum();
        let mean = sum / matrix.n_samples as f32;
        assert!(mean.abs() < 1e-5, "expected zero mean, got {mean}");
    }

    #[test]
    fn feature_matrix_get_returns_correct_value() {
        let ds = make_dataset(&[10.0, 20.0]);
        let pipeline = FeaturePipeline::new();
        let matrix = pipeline.transform(&ds);
        assert_eq!(matrix.get(0, 0), Some(10.0));
        assert_eq!(matrix.get(1, 0), Some(20.0));
        assert_eq!(matrix.get(2, 0), None);
    }
}
