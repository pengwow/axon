//! TDD 第五轮：CounterfactualGenerator
//!
//! 反事实解释：修改若干特征，观察预测/置信度变化。
//! 业务规则：只在置信度变化超过阈值时保留反事实。

use axon_explain::counterfactual::{CounterfactualConfig, CounterfactualGenerator};
use axon_explain::traits::{Explainer, ModelPredictor};
use axon_explain::types::{
    ActionAttribution, ActionSnapshot, ContributionDirection, CounterfactualExplanation,
    Explanation, FeatureContribution,
};
use std::collections::HashMap;

// ─── 测试模型与 Explainer ─────────────────────────────────

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

struct TestExplainer {
    model: LinearModel,
    feature_names: Vec<String>,
}

impl Explainer for TestExplainer {
    fn explain(
        &self,
        observation: &HashMap<String, f64>,
        _action: &ActionSnapshot,
    ) -> Result<Explanation, axon_explain::error::ExplainabilityError> {
        let features: Vec<f64> = self
            .feature_names
            .iter()
            .map(|n| *observation.get(n).unwrap_or(&0.0))
            .collect();
        let pred = self.model.predict(&features)[0];
        // confidence 随 prediction 绝对值变化：sigmoid-like
        let confidence = 1.0 / (1.0 + (-pred.abs() / 100.0).exp());
        let mut fi = HashMap::new();
        let mut contribs = Vec::new();
        for (i, name) in self.feature_names.iter().enumerate() {
            let shap = self.model.coefficients[i] * features[i];
            fi.insert(name.clone(), shap.abs());
            contribs.push(FeatureContribution {
                feature_name: name.clone(),
                shap_value: shap,
                feature_value: features[i],
                direction: ContributionDirection::from_shap(shap),
            });
        }
        Ok(Explanation {
            id: "e".into(),
            observation_id: "o".into(),
            action: ActionSnapshot {
                position_size: pred,
                entry_price: 0.0,
                stop_loss: 0.0,
                take_profit: 0.0,
                order_type: "limit".into(),
            },
            feature_importance: fi,
            action_attributions: vec![ActionAttribution::from_contributions(
                "position_size".into(),
                pred,
                0.0,
                contribs,
            )],
            attention_weights: None,
            counterfactuals: vec![],
            summary: "".into(),
            confidence,
            generated_at: chrono::Utc::now(),
        })
    }

    fn explain_action_dimension(
        &self,
        observation: &HashMap<String, f64>,
        _action: &ActionSnapshot,
        _dim: &str,
    ) -> Result<ActionAttribution, axon_explain::error::ExplainabilityError> {
        let exp = self.explain(observation, _action)?;
        Ok(exp.action_attributions.into_iter().next().unwrap())
    }

    fn get_attention_weights(
        &self,
        _: &HashMap<String, f64>,
    ) -> Option<Vec<axon_explain::types::AttentionWeights>> {
        None
    }

    fn generate_counterfactuals(
        &self,
        _observation: &HashMap<String, f64>,
        _action: &ActionSnapshot,
        _max: usize,
    ) -> Vec<CounterfactualExplanation> {
        vec![]
    }
}

// ─── CounterfactualConfig ──────────────────────────────────

#[test]
fn test_counterfactual_config_defaults() {
    let c = CounterfactualConfig::default();
    assert!(c.max_changes > 0);
    assert!(c.confidence_threshold > 0.0 && c.confidence_threshold < 1.0);
    assert!(c.step_size > 0.0 && c.step_size <= 1.0);
}

#[test]
fn test_counterfactual_config_builder() {
    let c = CounterfactualConfig::new()
        .with_max_changes(5)
        .with_step_size(0.3)
        .with_confidence_threshold(0.1);
    assert_eq!(c.max_changes, 5);
    assert!((c.step_size - 0.3).abs() < 1e-9);
    assert!((c.confidence_threshold - 0.1).abs() < 1e-9);
}

// ─── CounterfactualGenerator ───────────────────────────────

#[test]
fn test_generator_creates_with_config() {
    let model = LinearModel::new(vec![0.5, 0.3], 0.0);
    let generator = CounterfactualGenerator::with_feature_names(
        Box::new(model),
        vec!["f1".into(), "f2".into()],
        CounterfactualConfig::default(),
    );
    let _ = generator;
}

#[test]
fn test_generator_returns_counterfactuals() {
    let model = LinearModel::new(vec![0.5, 0.3, 0.2], 0.0);
    let generator = CounterfactualGenerator::with_feature_names(
        Box::new(model),
        vec!["f1".into(), "f2".into(), "f3".into()],
        CounterfactualConfig::new()
            .with_max_changes(3)
            .with_confidence_threshold(0.01),
    );
    let explainer = TestExplainer {
        model: LinearModel::new(vec![0.5, 0.3, 0.2], 0.0),
        feature_names: vec!["f1".into(), "f2".into(), "f3".into()],
    };

    let mut obs = HashMap::new();
    obs.insert("f1".into(), 10.0);
    obs.insert("f2".into(), 20.0);
    obs.insert("f3".into(), 30.0);
    let action = ActionSnapshot {
        position_size: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };

    let cfs = generator.generate(&obs, &action, &explainer);
    // 3 个特征，每个都可能产生反事实
    assert!(cfs.len() <= 3);
    for cf in &cfs {
        assert_eq!(cf.changed_features.len(), 1);
        assert!(cf.narrative.contains("如果"));
        assert!(cf.narrative.contains("变为"));
    }
}

/// 关键测试（来自设计）：反事实必须满足"修改后预测与原始预测差异显著"
#[test]
fn test_counterfactual_predicts_different_action() {
    let model = LinearModel::new(vec![1.0, 0.0, 0.0], 0.0); // 只依赖 f1
    let generator = CounterfactualGenerator::with_feature_names(
        Box::new(model),
        vec!["f1".into(), "f2".into(), "f3".into()],
        CounterfactualConfig::new()
            .with_max_changes(3)
            .with_confidence_threshold(0.01),
    );
    let explainer = TestExplainer {
        model: LinearModel::new(vec![1.0, 0.0, 0.0], 0.0),
        feature_names: vec!["f1".into(), "f2".into(), "f3".into()],
    };

    let mut obs = HashMap::new();
    obs.insert("f1".into(), 100.0);
    obs.insert("f2".into(), 50.0);
    obs.insert("f3".into(), 30.0);
    let action = ActionSnapshot {
        position_size: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };
    let cfs = generator.generate(&obs, &action, &explainer);

    // 修改 f1 必然产生显著变化（coef=1）
    let f1_cf = cfs.iter().find(|cf| cf.changed_features[0] == "f1");
    assert!(f1_cf.is_some(), "修改最关键特征 f1 应产生反事实");
}

/// 关键测试（来自设计）：超过 max_changes 必须截断
#[test]
fn test_counterfactual_respects_max_changes() {
    let model = LinearModel::new(vec![0.1; 5], 0.0);
    let generator = CounterfactualGenerator::with_feature_names(
        Box::new(model),
        vec![
            "f1".into(),
            "f2".into(),
            "f3".into(),
            "f4".into(),
            "f5".into(),
        ],
        CounterfactualConfig::new()
            .with_max_changes(2)
            .with_confidence_threshold(0.0),
    );
    let explainer = TestExplainer {
        model: LinearModel::new(vec![0.1; 5], 0.0),
        feature_names: vec![
            "f1".into(),
            "f2".into(),
            "f3".into(),
            "f4".into(),
            "f5".into(),
        ],
    };
    let mut obs = HashMap::new();
    for (i, n) in ["f1", "f2", "f3", "f4", "f5"].iter().enumerate() {
        obs.insert(n.to_string(), (i as f64 + 1.0) * 10.0);
    }
    let action = ActionSnapshot {
        position_size: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };
    let cfs = generator.generate(&obs, &action, &explainer);
    assert!(cfs.len() <= 2);
}

/// 反事实叙事必须包含原始值、新值和置信度变化
#[test]
fn test_counterfactual_narrative_includes_values() {
    let model = LinearModel::new(vec![0.5, 0.3], 0.0);
    let generator = CounterfactualGenerator::with_feature_names(
        Box::new(model),
        vec!["f1".into(), "f2".into()],
        CounterfactualConfig::new()
            .with_max_changes(2)
            .with_confidence_threshold(0.0),
    );
    let explainer = TestExplainer {
        model: LinearModel::new(vec![0.5, 0.3], 0.0),
        feature_names: vec!["f1".into(), "f2".into()],
    };
    let mut obs = HashMap::new();
    obs.insert("f1".into(), 100.0);
    obs.insert("f2".into(), 100.0);
    let action = ActionSnapshot {
        position_size: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        order_type: "limit".into(),
    };
    let cfs = generator.generate(&obs, &action, &explainer);

    assert!(!cfs.is_empty());
    let narrative = &cfs[0].narrative;
    // 包含原始值和新值的数字
    assert!(
        narrative.contains("100.00"),
        "叙事应包含原始值 100.00: {}",
        narrative
    );
    assert!(
        narrative.contains("50.00"),
        "叙事应包含新值 50.00（步长 1.0 均值拉近 50%）: {}",
        narrative
    );
}
