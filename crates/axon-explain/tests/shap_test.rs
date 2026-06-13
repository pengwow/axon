//! TDD 第四轮：KernelSHAP 纯 Rust 实现
//!
//! KernelSHAP 是模型无关的 SHAP 算法，通过加权线性回归近似 Shapley 值。
//! 对于线性模型，结果应与解析解 phi_i = coef_i * (x_i - E[x_i]) 高度一致。

use axon_explain::shap::KernelSHAP;
use axon_explain::traits::ModelPredictor;

// ─── 线性模型（已知解析解） ─────────────────────────────────

struct LinearModel {
    coefficients: Vec<f64>,
    bias: f64,
}

impl LinearModel {
    fn new(coefs: Vec<f64>, bias: f64) -> Self {
        Self {
            coefficients: coefs,
            bias,
        }
    }
}

impl ModelPredictor for LinearModel {
    fn predict(&self, features: &[f64]) -> Vec<f64> {
        let value: f64 = self.bias
            + self
                .coefficients
                .iter()
                .zip(features)
                .map(|(c, x)| c * x)
                .sum::<f64>();
        vec![value]
    }
}

// ─── KernelSHAP 基础构造 ──────────────────────────────────

#[test]
fn test_kernel_shap_constructs_with_background() {
    let model = LinearModel::new(vec![0.5, -0.3, 0.1], 1.0);
    let background = vec![vec![1.0, 2.0, 3.0], vec![2.0, 3.0, 4.0]];
    let _explainer = KernelSHAP::new(Box::new(model), background, 100);
}

/// KernelSHAP 必须拒绝空背景数据集
#[test]
fn test_kernel_shap_rejects_empty_background() {
    let model = LinearModel::new(vec![0.5], 0.0);
    let result = KernelSHAP::try_new(Box::new(model), vec![], 100);
    assert!(result.is_err());
}

// ─── KernelSHAP 精度：单特征线性模型 ──────────────────────────

#[test]
fn test_kernel_shap_linear_single_feature_recovers_coefficient() {
    // f(x) = 2 * x
    let model = LinearModel::new(vec![2.0], 0.0);
    let background = vec![vec![0.0], vec![1.0], vec![2.0], vec![3.0]];
    let explainer = KernelSHAP::try_new(Box::new(model), background, 200).unwrap();

    // 观察 x=5: phi = 2 * (5 - 1.5) = 7
    let shap = explainer.compute_shap(&[5.0]);
    assert_eq!(shap.len(), 1);
    assert!(
        (shap[0] - 7.0).abs() < 0.5,
        "单特征 SHAP 误差过大: 实际={}, 期望≈7.0",
        shap[0]
    );
}

/// 关键测试（来自设计）：SHAP 值之和 ≈ (predicted - base)
#[test]
fn test_kernel_shap_local_accuracy() {
    let model = LinearModel::new(vec![0.5, -0.3, 0.1], 1.0);
    let background = vec![
        vec![1.0, 2.0, 3.0],
        vec![2.0, 3.0, 4.0],
        vec![3.0, 4.0, 5.0],
    ];
    let explainer = KernelSHAP::try_new(Box::new(model), background.clone(), 500).unwrap();

    let observation = vec![10.0, 20.0, 30.0];
    let shap = explainer.compute_shap(&observation);
    assert_eq!(shap.len(), 3);

    // 局部精度：sum(phi) ≈ f(x) - E[f(X)]
    let mean_bg: Vec<f64> = (0..3)
        .map(|i| background.iter().map(|r| r[i]).sum::<f64>() / background.len() as f64)
        .collect();
    let base_pred = 1.0 + 0.5 * mean_bg[0] + (-0.3) * mean_bg[1] + 0.1 * mean_bg[2];
    let pred = 1.0 + 0.5 * 10.0 + (-0.3) * 20.0 + 0.1 * 30.0;
    let expected_sum = pred - base_pred;
    let actual_sum: f64 = shap.iter().sum();

    // 容差较大因为是采样近似，但应 < 1.0
    assert!(
        (actual_sum - expected_sum).abs() < 1.0,
        "SHAP 局部精度不足: actual={}, expected={}",
        actual_sum,
        expected_sum
    );
}

/// 关键测试（来自设计）：对线性模型，SHAP 值与解析解方向一致
#[test]
fn test_kernel_shap_linear_recovers_sign() {
    let model = LinearModel::new(vec![0.5, -0.3, 0.1], 1.0);
    let background = vec![vec![1.0, 2.0, 3.0]; 20];
    let explainer = KernelSHAP::try_new(Box::new(model), background, 300).unwrap();

    // 特征值远大于背景均值 → 正 SHAP（系数为正）
    let shap = explainer.compute_shap(&[100.0, 100.0, 100.0]);
    assert!(
        shap[0] > 0.0,
        "正系数 + 远高于均值 → 应有正 SHAP，实际={}",
        shap[0]
    );
    // 系数为负
    assert!(
        shap[1] < 0.0,
        "负系数 + 远高于均值 → 应有负 SHAP，实际={}",
        shap[1]
    );
}

/// KernelSHAP 拒绝特征数量不匹配
#[test]
fn test_kernel_shap_rejects_feature_mismatch() {
    let model = LinearModel::new(vec![0.5, 0.3], 0.0);
    let background = vec![vec![1.0, 2.0]];
    let explainer = KernelSHAP::try_new(Box::new(model), background, 100).unwrap();
    let result = explainer.try_compute_shap(&[1.0]); // 特征数量不足
    assert!(result.is_err());
}

/// SHAP 归因对所有特征维度返回正确长度
#[test]
fn test_kernel_shap_returns_correct_dim() {
    let model = LinearModel::new(vec![0.1; 5], 0.0);
    let background = vec![vec![1.0; 5]; 10];
    let explainer = KernelSHAP::try_new(Box::new(model), background, 100).unwrap();
    let shap = explainer.compute_shap(&[2.0; 5]);
    assert_eq!(shap.len(), 5);
}
