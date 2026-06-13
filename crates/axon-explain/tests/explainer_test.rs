//! TDD 第二/三轮：ExplainabilityError + Explainer trait + MockExplainer
//!
//! Explainer 是抽象层 - 必须支持 explain / explain_action_dimension /
//! get_attention_weights / generate_counterfactuals 四个能力。

use axon_explain::error::ExplainabilityError;
use axon_explain::traits::{Explainer, ModelPredictor};
use axon_explain::types::{
    ActionAttribution, ActionSnapshot, ContributionDirection, CounterfactualExplanation,
    Explanation, FeatureContribution,
};
use chrono::Utc;
use std::collections::HashMap;

// ─── ExplainabilityError ────────────────────────────────────

#[test]
fn test_error_invalid_dimension_displays_name() {
    let e = ExplainabilityError::InvalidDimension("position_size".into());
    assert!(e.to_string().contains("position_size"));
}

#[test]
fn test_error_feature_mismatch_displays_expected_and_actual() {
    let e = ExplainabilityError::FeatureMismatch {
        expected: 6,
        actual: 4,
    };
    let msg = e.to_string();
    assert!(msg.contains("6"));
    assert!(msg.contains("4"));
}

#[test]
fn test_error_python_interop_displays_message() {
    let e = ExplainabilityError::PythonInterop("pyo3 init failed".into());
    assert!(e.to_string().contains("pyo3 init failed"));
}

#[test]
fn test_error_shap_computation_displays_message() {
    let e = ExplainabilityError::SHAPComputationFailed("singular matrix".into());
    assert!(e.to_string().contains("singular matrix"));
}

#[test]
fn test_error_model_not_loaded_displays_path() {
    let e = ExplainabilityError::ModelNotLoaded("model.pt".into());
    assert!(e.to_string().contains("model.pt"));
}

/// 错误分类：可恢复 vs 不可恢复
#[test]
fn test_error_is_recoverable() {
    assert!(ExplainabilityError::SHAPComputationFailed("x".into()).is_recoverable());
    assert!(ExplainabilityError::AttentionExtractionFailed("x".into()).is_recoverable());
    assert!(ExplainabilityError::ReportGenerationFailed("x".into()).is_recoverable());

    assert!(!ExplainabilityError::ModelNotLoaded("x".into()).is_recoverable());
    assert!(
        !ExplainabilityError::FeatureMismatch {
            expected: 1,
            actual: 2
        }
        .is_recoverable()
    );
    assert!(!ExplainabilityError::InvalidDimension("x".into()).is_recoverable());
}

// ─── Explainer trait：MockExplainer 实现 ──────────────────────

/// 线性模型：prediction = sum(coef * x)
struct LinearModel {
    coefficients: Vec<f64>,
    bias: f64,
}

impl LinearModel {
    fn new(coefficients: Vec<f64>, bias: f64) -> Self {
        Self { coefficients, bias }
    }
}

impl ModelPredictor for LinearModel {
    fn predict(&self, features: &[f64]) -> Vec<f64> {
        // 单输出：position_size
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

/// MockExplainer：基于线性模型的精确 SHAP（解析解）
struct MockLinearExplainer {
    model: LinearModel,
    feature_names: Vec<String>,
    base_value: f64,
    background_mean: Vec<f64>,
}

impl MockLinearExplainer {
    fn new(model: LinearModel, feature_names: Vec<String>, background: Vec<f64>) -> Self {
        let base = model.predict(&background)[0];
        Self {
            model,
            feature_names,
            base_value: base,
            background_mean: background,
        }
    }
}

impl Explainer for MockLinearExplainer {
    fn explain(
        &self,
        observation: &HashMap<String, f64>,
        _action: &ActionSnapshot,
    ) -> Result<Explanation, ExplainabilityError> {
        // 线性模型的 SHAP 解析解：phi_i = coef_i * (x_i - E[x_i])
        let mut contributions = Vec::new();
        let mut feature_importance = HashMap::new();
        for (i, name) in self.feature_names.iter().enumerate() {
            let x = *observation
                .get(name)
                .ok_or(ExplainabilityError::FeatureMismatch {
                    expected: self.feature_names.len(),
                    actual: observation.len(),
                })?;
            let shap = self.model.coefficients[i] * (x - self.background_mean[i]);
            contributions.push(FeatureContribution {
                feature_name: name.clone(),
                shap_value: shap,
                feature_value: x,
                direction: ContributionDirection::from_shap(shap),
            });
            feature_importance.insert(name.clone(), shap.abs());
        }

        // 关键测试：SHAP 之和应近似 (predicted - base)
        let pred = self.model.predict(
            &self
                .feature_names
                .iter()
                .map(|n| *observation.get(n).unwrap())
                .collect::<Vec<_>>(),
        )[0];
        let sum: f64 = contributions.iter().map(|c| c.shap_value).sum();
        assert!(
            (sum - (pred - self.base_value)).abs() < 1e-9,
            "SHAP 局部精度失败: sum={}, pred-base={}",
            sum,
            pred - self.base_value
        );

        Ok(Explanation {
            id: "exp_001".to_string(),
            observation_id: "obs_001".to_string(),
            action: ActionSnapshot {
                position_size: pred,
                entry_price: 0.0,
                stop_loss: 0.0,
                take_profit: 0.0,
                order_type: "limit".into(),
            },
            feature_importance,
            action_attributions: vec![ActionAttribution::from_contributions(
                "position_size".to_string(),
                pred,
                self.base_value,
                contributions,
            )],
            attention_weights: None,
            counterfactuals: vec![],
            summary: format!("预测 {:.4}, 基准 {:.4}", pred, self.base_value),
            confidence: 0.9,
            generated_at: Utc::now(),
        })
    }

    fn explain_action_dimension(
        &self,
        observation: &HashMap<String, f64>,
        _action: &ActionSnapshot,
        dimension: &str,
    ) -> Result<ActionAttribution, ExplainabilityError> {
        if dimension != "position_size" {
            return Err(ExplainabilityError::InvalidDimension(dimension.into()));
        }
        let full = self.explain(observation, _action)?;
        full.action_attributions
            .into_iter()
            .next()
            .ok_or_else(|| ExplainabilityError::SHAPComputationFailed("no attribution".into()))
    }

    fn get_attention_weights(
        &self,
        _observation: &HashMap<String, f64>,
    ) -> Option<Vec<axon_explain::types::AttentionWeights>> {
        // 线性模型无注意力
        None
    }

    fn generate_counterfactuals(
        &self,
        observation: &HashMap<String, f64>,
        _action: &ActionSnapshot,
        max_changes: usize,
    ) -> Vec<CounterfactualExplanation> {
        // 简化：前 N 个特征向均值拉近 50%
        let mut cfs = Vec::new();
        let original_pred = self.model.predict(
            &self
                .feature_names
                .iter()
                .map(|n| *observation.get(n).unwrap_or(&0.0))
                .collect::<Vec<_>>(),
        )[0];

        for (i, name) in self.feature_names.iter().take(max_changes).enumerate() {
            let x = *observation.get(name).unwrap_or(&0.0);
            let mean = self.background_mean[i];
            let new_val = x + (mean - x) * 0.5;
            let mut modified = observation.clone();
            modified.insert(name.clone(), new_val);
            let new_pred = self.model.predict(
                &self
                    .feature_names
                    .iter()
                    .map(|n| *modified.get(n).unwrap_or(&0.0))
                    .collect::<Vec<_>>(),
            )[0];

            cfs.push(CounterfactualExplanation {
                original_action: ActionSnapshot {
                    position_size: original_pred,
                    entry_price: 0.0,
                    stop_loss: 0.0,
                    take_profit: 0.0,
                    order_type: "limit".into(),
                },
                modified_action: ActionSnapshot {
                    position_size: new_pred,
                    entry_price: 0.0,
                    stop_loss: 0.0,
                    take_profit: 0.0,
                    order_type: "limit".into(),
                },
                changed_features: vec![name.clone()],
                original_confidence: 0.9,
                new_confidence: 0.8,
                narrative: format!(
                    "如果 {} 从 {:.4} 变为 {:.4}, 预测从 {:.4} 变为 {:.4}",
                    name, x, new_val, original_pred, new_pred
                ),
            });
        }

        cfs
    }
}

// ─── Explainer trait：行为测试 ──────────────────────────────

#[test]
fn test_explainer_returns_action_attribution() {
    let model = LinearModel::new(vec![0.5, -0.3, 0.1], 1.0);
    let bg = vec![10.0, 20.0, 30.0];
    let explainer = MockLinearExplainer::new(
        model,
        vec!["price".into(), "volume".into(), "rsi".into()],
        bg,
    );

    let mut obs = HashMap::new();
    obs.insert("price".into(), 12.0);
    obs.insert("volume".into(), 25.0);
    obs.insert("rsi".into(), 35.0);

    let action = ActionSnapshot {
        position_size: 1.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };

    let attr = explainer
        .explain_action_dimension(&obs, &action, "position_size")
        .unwrap();
    assert_eq!(attr.dimension, "position_size");
    assert!(attr.feature_contributions.len() == 3);
}

/// 关键测试（来自设计）：SHAP 值之和应接近 (predicted - base)
#[test]
fn test_shap_values_sum_to_prediction_minus_base() {
    let model = LinearModel::new(vec![0.5, -0.3, 0.1, 0.2], 1.0);
    let bg = vec![10.0, 20.0, 30.0, 40.0];
    let explainer = MockLinearExplainer::new(
        model,
        vec!["f1".into(), "f2".into(), "f3".into(), "f4".into()],
        bg,
    );

    let mut obs = HashMap::new();
    obs.insert("f1".into(), 15.0); // +5 * 0.5 = +2.5
    obs.insert("f2".into(), 25.0); // +5 * -0.3 = -1.5
    obs.insert("f3".into(), 35.0); // +5 * 0.1 = +0.5
    obs.insert("f4".into(), 45.0); // +5 * 0.2 = +1.0
    // sum = 2.5 - 1.5 + 0.5 + 1.0 = 2.5

    let action = ActionSnapshot {
        position_size: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };
    let exp = explainer.explain(&obs, &action).unwrap();

    let sum: f64 = exp.action_attributions[0]
        .feature_contributions
        .iter()
        .map(|c| c.shap_value)
        .sum();
    let expected =
        exp.action_attributions[0].predicted_value - exp.action_attributions[0].base_value;
    assert!((sum - expected).abs() < 1e-9);
}

/// Explainer 拒绝非法维度
#[test]
fn test_explainer_rejects_invalid_dimension() {
    let model = LinearModel::new(vec![0.5], 0.0);
    let explainer = MockLinearExplainer::new(model, vec!["x".into()], vec![0.0]);
    let mut obs = HashMap::new();
    obs.insert("x".into(), 1.0);
    let action = ActionSnapshot {
        position_size: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };
    let err = explainer
        .explain_action_dimension(&obs, &action, "unknown_dim")
        .unwrap_err();
    assert!(matches!(err, ExplainabilityError::InvalidDimension(_)));
}

/// Explainer 检测特征数量不匹配
#[test]
fn test_explainer_detects_feature_mismatch() {
    let model = LinearModel::new(vec![0.5, 0.3], 0.0);
    let explainer = MockLinearExplainer::new(model, vec!["f1".into(), "f2".into()], vec![0.0, 0.0]);
    let mut obs = HashMap::new();
    obs.insert("f1".into(), 1.0); // 缺 f2
    let action = ActionSnapshot {
        position_size: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };
    let err = explainer.explain(&obs, &action).unwrap_err();
    assert!(matches!(err, ExplainabilityError::FeatureMismatch { .. }));
}

/// Explainer 生成反事实
#[test]
fn test_explainer_generates_counterfactuals() {
    let model = LinearModel::new(vec![0.5, -0.3], 0.0);
    let explainer = MockLinearExplainer::new(model, vec!["f1".into(), "f2".into()], vec![0.0, 0.0]);
    let mut obs = HashMap::new();
    obs.insert("f1".into(), 10.0);
    obs.insert("f2".into(), 20.0);
    let action = ActionSnapshot {
        position_size: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };

    let cfs = explainer.generate_counterfactuals(&obs, &action, 2);
    assert_eq!(cfs.len(), 2);
    for cf in &cfs {
        assert_eq!(cf.changed_features.len(), 1);
        assert!(cf.narrative.contains("如果"));
    }
}
