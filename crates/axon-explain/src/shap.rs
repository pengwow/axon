//! KernelSHAP 纯 Rust 实现
//!
//! KernelSHAP 是模型无关的 SHAP 算法：
//! 1. 在背景数据上构建合成样本（部分掩码）
//! 2. 使用 SHAP 核权重加权
//! 3. 拟合加权线性回归
//! 4. 回归系数即为 SHAP 值近似
//!
//! 参考：Lundberg & Lee (2017) "A Unified Approach to Interpreting Model Predictions"

use rand::seq::SliceRandom;
use rand::{SeedableRng, rngs::StdRng};
use tracing::debug;

use crate::error::ExplainabilityError;
use crate::traits::ModelPredictor;

/// KernelSHAP 解释器
pub struct KernelSHAP {
    /// 底层模型
    model: Box<dyn ModelPredictor>,
    /// 背景数据集（n_samples × n_features,API 保留以便后续可视化/复用,目前仅 background_mean 被实际使用)
    #[allow(dead_code)]
    background: Vec<Vec<f64>>,
    /// 背景数据均值（n_features）
    background_mean: Vec<f64>,
    /// 采样数（拟合回归的样本数）
    n_samples: usize,
}

impl KernelSHAP {
    /// 创建 KernelSHAP
    pub fn new(
        model: Box<dyn ModelPredictor>,
        background: Vec<Vec<f64>>,
        n_samples: usize,
    ) -> Self {
        let background_mean = Self::compute_mean(&background);
        let n_samples = n_samples.max(2);
        Self {
            model,
            background,
            background_mean,
            n_samples,
        }
    }

    /// 尝试创建，返回错误而非 panic
    pub fn try_new(
        model: Box<dyn ModelPredictor>,
        background: Vec<Vec<f64>>,
        n_samples: usize,
    ) -> Result<Self, ExplainabilityError> {
        if background.is_empty() {
            return Err(ExplainabilityError::SHAPComputationFailed(
                "background dataset is empty".into(),
            ));
        }
        let expected = background[0].len();
        if expected == 0 {
            return Err(ExplainabilityError::SHAPComputationFailed(
                "background has zero features".into(),
            ));
        }
        if background.iter().any(|row| row.len() != expected) {
            return Err(ExplainabilityError::SHAPComputationFailed(
                "inconsistent background feature dimensions".into(),
            ));
        }
        Ok(Self::new(model, background, n_samples))
    }

    /// 计算 SHAP 值
    pub fn compute_shap(&self, observation: &[f64]) -> Vec<f64> {
        self.try_compute_shap(observation)
            .expect("KernelSHAP::compute_shap called with invalid input")
    }

    /// 尝试计算 SHAP 值
    pub fn try_compute_shap(&self, observation: &[f64]) -> Result<Vec<f64>, ExplainabilityError> {
        let n_features = self.background_mean.len();
        if observation.len() != n_features {
            return Err(ExplainabilityError::FeatureMismatch {
                expected: n_features,
                actual: observation.len(),
            });
        }

        let mut rng = StdRng::seed_from_u64(42);
        let coalitions = self.sample_coalitions(n_features, &mut rng);
        let predictions = self.evaluate_coalitions(observation, &coalitions);
        let weights = self.shap_kernel_weights(&coalitions);

        // 设计矩阵 X: 第一列是 1（截距），后续列为联盟掩码 z ∈ {0,1}^M
        // 这是 KernelSHAP 的标准形式：回归系数即为 SHAP 值
        let n = coalitions.len();
        let mut x = vec![vec![0.0; n_features + 1]; n];
        for (i, coalition) in coalitions.iter().enumerate() {
            x[i][0] = 1.0;
            for (j, &in_coalition) in coalition.iter().enumerate() {
                x[i][j + 1] = if in_coalition { 1.0 } else { 0.0 };
            }
        }

        // 加权最小二乘
        let coefs = self.weighted_linear_regression(&x, &predictions, &weights);
        // 第一个系数是截距（base value），后续是 SHAP 值
        Ok(coefs[1..].to_vec())
    }

    /// 采样联盟：每个联盟是一个 bool 数组，表示哪些特征"在场"
    fn sample_coalitions(&self, n_features: usize, rng: &mut StdRng) -> Vec<Vec<bool>> {
        // 2^M 中 M 是特征数；M 较大时采样固定数量
        let n_samples = self.n_samples.min(1 << n_features.max(1));
        let mut coalitions = Vec::with_capacity(n_samples + 2);

        // 关键：必须包含全 0（无特征）和全 1（所有特征）
        coalitions.push(vec![false; n_features]);
        coalitions.push(vec![true; n_features]);

        // 随机采样其他联盟
        let total = 1usize << n_features;
        if total <= self.n_samples {
            // 全部枚举
            for mask in 0..total {
                if mask == 0 || mask == total - 1 {
                    continue;
                }
                let coalition: Vec<bool> = (0..n_features).map(|i| (mask >> i) & 1 == 1).collect();
                coalitions.push(coalition);
            }
        } else {
            // 随机采样（不重复）
            let mut all_masks: Vec<usize> = (0..total).collect();
            all_masks.shuffle(rng);
            for &mask in all_masks.iter().take(self.n_samples) {
                if mask == 0 || mask == total - 1 {
                    continue;
                }
                let coalition: Vec<bool> = (0..n_features).map(|i| (mask >> i) & 1 == 1).collect();
                coalitions.push(coalition);
            }
        }

        coalitions
    }

    /// 评估每个联盟的模型输出
    fn evaluate_coalitions(&self, observation: &[f64], coalitions: &[Vec<bool>]) -> Vec<f64> {
        coalitions
            .iter()
            .map(|coalition| {
                let mut features = vec![0.0; observation.len()];
                for (i, &in_coalition) in coalition.iter().enumerate() {
                    features[i] = if in_coalition {
                        observation[i]
                    } else {
                        self.background_mean[i]
                    };
                }
                let preds = self.model.predict(&features);
                preds[0]
            })
            .collect()
    }

    /// SHAP 核权重：w(z) = (M-1) / (C(M, |z|) * |z| * (M - |z|))
    fn shap_kernel_weights(&self, coalitions: &[Vec<bool>]) -> Vec<f64> {
        let m = coalitions.first().map(|c| c.len()).unwrap_or(1) as f64;
        let mut weights = Vec::with_capacity(coalitions.len());
        for coalition in coalitions {
            let z = coalition.iter().filter(|&&b| b).count() as f64;
            let binom = binomial_coefficient(m as usize, z as usize);
            if z == 0.0 || z == m || binom == 0.0 {
                // 边界情况：无穷大权重，用大值
                weights.push(1e6);
            } else {
                let w = (m - 1.0) / (binom * z * (m - z));
                weights.push(w);
            }
        }
        weights
    }

    /// 加权线性回归（最小二乘）
    /// 使用正规方程：(X^T W X) β = X^T W y
    fn weighted_linear_regression(&self, x: &[Vec<f64>], y: &[f64], weights: &[f64]) -> Vec<f64> {
        let n = x.len();
        let p = x[0].len();
        assert_eq!(y.len(), n);
        assert_eq!(weights.len(), n);

        // 构建 X^T W X 和 X^T W y
        let mut xtwx = vec![vec![0.0; p]; p];
        let mut xtwy = vec![0.0; p];

        for i in 0..n {
            let w = weights[i];
            for j in 0..p {
                xtwy[j] += w * x[i][j] * y[i];
                for k in 0..p {
                    xtwx[j][k] += w * x[i][j] * x[i][k];
                }
            }
        }

        // Tikhonov 正则化以保证数值稳定
        for (j, row) in xtwx.iter_mut().enumerate().take(p) {
            row[j] += 1e-6;
        }

        solve_linear_system(&xtwx, &xtwy).unwrap_or_else(|_| vec![0.0; p])
    }

    /// 计算列均值
    fn compute_mean(data: &[Vec<f64>]) -> Vec<f64> {
        if data.is_empty() {
            return vec![];
        }
        let n_features = data[0].len();
        let n = data.len() as f64;
        (0..n_features)
            .map(|i| data.iter().map(|row| row[i]).sum::<f64>() / n)
            .collect()
    }
}

/// 二项式系数
fn binomial_coefficient(n: usize, k: usize) -> f64 {
    if k > n {
        return 0.0;
    }
    let k = k.min(n - k);
    let mut result = 1.0_f64;
    for i in 0..k {
        result *= (n - i) as f64 / (i + 1) as f64;
    }
    result
}

/// 高斯消去法求解 Ax = b（A 为 n×n 矩阵）
fn solve_linear_system(a: &[Vec<f64>], b: &[f64]) -> Result<Vec<f64>, &'static str> {
    let n = a.len();
    if n == 0 || a[0].len() != n || b.len() != n {
        return Err("dimension mismatch");
    }

    // 构造增广矩阵
    let mut aug: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let mut row = vec![0.0; n + 1];
            row[..n].copy_from_slice(&a[i][..n]);
            row[n] = b[i];
            row
        })
        .collect();

    // 前向消元
    for i in 0..n {
        // 选主元
        let mut max_row = i;
        for k in (i + 1)..n {
            if aug[k][i].abs() > aug[max_row][i].abs() {
                max_row = k;
            }
        }
        aug.swap(i, max_row);

        if aug[i][i].abs() < 1e-12 {
            debug!("矩阵奇异，使用 fallback");
            return Err("singular matrix");
        }

        // 消去
        for k in (i + 1)..n {
            let factor = aug[k][i] / aug[i][i];
            // 复制 pivot 行到本地 Vec,避免同时借用 aug[k] 与 aug[i] 触发的借用冲突
            let pivot_row: Vec<f64> = aug[i][i..=n].to_vec();
            for (j, t) in aug[k][i..=n].iter_mut().enumerate() {
                *t -= factor * pivot_row[j];
            }
        }
    }

    // 回代
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = aug[i][n];
        for j in (i + 1)..n {
            sum -= aug[i][j] * x[j];
        }
        x[i] = sum / aug[i][i];
    }

    Ok(x)
}
