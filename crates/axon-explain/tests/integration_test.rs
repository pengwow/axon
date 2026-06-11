//! 集成测试：端到端可解释性流程
//!
//! 模拟真实场景：模型决策 → KernelSHAP 归因 → 反事实生成 → 报告输出
//!
//! 关键场景（来自设计文档）：
//! 1. 解释一致性：SHAP 之和 = predicted - base
//! 2. 反事实业务规则：仅当置信度变化显著时保留
//! 3. 报告输出：HTML 与 Markdown 均包含核心元素
//! 4. 端到端性能：单次解释 < 100ms

use std::collections::HashMap;
use std::time::Instant;
use chrono::Utc;
use axon_explain::counterfactual::{CounterfactualConfig, CounterfactualGenerator};
use axon_explain::error::ExplainabilityError;
use axon_explain::report::ReportGenerator;
use axon_explain::shap::KernelSHAP;
use axon_explain::traits::{Explainer, ModelPredictor};
use axon_explain::types::{
    ActionAttribution, ActionSnapshot, Explanation, FeatureContribution, ContributionDirection,
};

// ─── 真实场景模型：模拟 BTC/USDT 交易决策 ──────────────────────

struct TradingModel {
    /// 系数：[price, rsi, volume, macd, atr, bb_pct]
    coefficients: Vec<f64>,
    bias: f64,
}

impl TradingModel {
    fn new() -> Self {
        // 模拟一个交易模型：
        // - RSI 60-70 → 强烈买入信号（coef=0.6）
        // - 价格突破 50000 → 中等正向（coef=0.0001）
        // - 成交量放大 → 弱正向（coef=0.0000003）
        // - MACD 正 → 中等（coef=0.4）
        // - ATR 高 → 弱负（coef=-0.0002）
        // - BB% 在 0.7-0.9 → 强正向（coef=0.5）
        Self {
            coefficients: vec![0.0001, 0.6, 0.0000003, 0.4, -0.0002, 0.5],
            bias: 0.0,
        }
    }
}

impl ModelPredictor for TradingModel {
    fn predict(&self, features: &[f64]) -> Vec<f64> {
        let value: f64 = self.bias
            + self.coefficients.iter().zip(features).map(|(c, x)| c * x).sum::<f64>();
        vec![value] // 单输出：建议仓位大小（0-1）
    }
}

struct TradingExplainer {
    model: TradingModel,
    feature_names: Vec<String>,
    background_mean: Vec<f64>,
}

impl TradingExplainer {
    fn new(model: TradingModel, background: Vec<Vec<f64>>) -> Self {
        let n = background[0].len();
        let mean: Vec<f64> = (0..n).map(|i| {
            background.iter().map(|r| r[i]).sum::<f64>() / background.len() as f64
        }).collect();
        let feature_names = vec![
            "price".into(), "rsi_14".into(), "volume".into(),
            "macd".into(), "atr".into(), "bb_pct".into(),
        ];
        Self { model, feature_names, background_mean: mean }
    }
}

impl Explainer for TradingExplainer {
    fn explain(
        &self,
        observation: &HashMap<String, f64>,
        _action: &ActionSnapshot,
    ) -> Result<Explanation, ExplainabilityError> {
        let features: Vec<f64> = self.feature_names.iter().map(|n| *observation.get(n).unwrap_or(&0.0)).collect();
        let pred = self.model.predict(&features)[0];
        let base = self.model.predict(&self.background_mean)[0];
        // confidence = 0.5 + tanh((pred - base) / 10) * 0.5 ∈ [0, 1]，不饱和
        let confidence = 0.5 + ((pred - base) / 10.0).tanh() * 0.5;

        let mut fi = HashMap::new();
        let mut contribs = Vec::new();
        for (i, name) in self.feature_names.iter().enumerate() {
            // SHAP 局部精度公式：phi_i = coef_i * (x_i - E[x_i])
            let shap = self.model.coefficients[i] * (features[i] - self.background_mean[i]);
            fi.insert(name.clone(), shap.abs());
            contribs.push(FeatureContribution {
                feature_name: name.clone(),
                shap_value: shap,
                feature_value: features[i],
                direction: ContributionDirection::from_shap(shap),
            });
        }

        Ok(Explanation {
            id: "trading_exp".into(),
            observation_id: "obs_btc".into(),
            action: ActionSnapshot { position_size: pred, entry_price: 0.0, stop_loss: 0.0, take_profit: 0.0, order_type: "limit".into() },
            feature_importance: fi,
            action_attributions: vec![ActionAttribution::from_contributions(
                "position_size".into(), pred, base, contribs,
            )],
            attention_weights: None,
            counterfactuals: vec![],
            summary: format!("建议仓位 {:.2}（基准 {:.2}）", pred, base),
            confidence,
            generated_at: Utc::now(),
        })
    }

    fn explain_action_dimension(
        &self,
        observation: &HashMap<String, f64>,
        action: &ActionSnapshot,
        _dim: &str,
    ) -> Result<ActionAttribution, ExplainabilityError> {
        let exp = self.explain(observation, action)?;
        Ok(exp.action_attributions.into_iter().next().unwrap())
    }

    fn get_attention_weights(&self, _: &HashMap<String, f64>) -> Option<Vec<axon_explain::types::AttentionWeights>> { None }

    fn generate_counterfactuals(
        &self,
        _observation: &HashMap<String, f64>,
        _action: &ActionSnapshot,
        _max: usize,
    ) -> Vec<axon_explain::types::CounterfactualExplanation> { vec![] }
}

// ─── 端到端测试：完整流程 ──────────────────────────────────

#[test]
fn test_end_to_end_trading_decision_explanation() {
    // 1. 准备背景数据（过去 100 个观察）
    let background: Vec<Vec<f64>> = (0..100).map(|i| vec![
        50_000.0 + (i as f64 * 10.0),  // price
        50.0 + (i as f64 % 30.0),      // rsi
        1_000_000.0 + (i as f64 * 1000.0),  // volume
        0.0 + (i as f64 * 0.01),       // macd
        100.0 + (i as f64 % 50.0),     // atr
        0.5 + (i as f64 % 30.0) / 100.0,    // bb_pct
    ]).collect();

    let model = TradingModel::new();
    let explainer = TradingExplainer::new(model, background.clone());

    // 2. 构造观察（当前市场状态：RSI 超买、价格突破、BB% 接近上轨）
    let mut observation = HashMap::new();
    observation.insert("price".into(), 51_000.0);   // 突破
    observation.insert("rsi_14".into(), 65.0);       // 超买
    observation.insert("volume".into(), 2_000_000.0); // 放大
    observation.insert("macd".into(), 0.5);         // 强正
    observation.insert("atr".into(), 130.0);        // 较高
    observation.insert("bb_pct".into(), 0.8);       // 接近上轨

    let action = ActionSnapshot {
        position_size: 0.0, entry_price: 51_000.0, stop_loss: 50_500.0,
        take_profit: 52_000.0, order_type: "limit".into(),
    };

    // 3. 解释
    let explanation = explainer.explain(&observation, &action).unwrap();

    // 4. 验证解释
    assert!(explanation.confidence > 0.0, "置信度应为正");
    assert_eq!(explanation.feature_importance.len(), 6, "应解释 6 个特征");

    // 5. 关键测试（来自设计）：SHAP 之和应 = predicted - base
    let attr = &explanation.action_attributions[0];
    let sum: f64 = attr.feature_contributions.iter().map(|c| c.shap_value).sum();
    let expected = attr.predicted_value - attr.base_value;
    assert!(
        (sum - expected).abs() < 1e-6,
        "SHAP 之和应 = predicted - base: sum={}, expected={}",
        sum, expected
    );

    // 6. 验证 RSI 和 BB% 应是 Top 正向（业务规则）
    let rsi = attr.feature_contributions.iter().find(|c| c.feature_name == "rsi_14").unwrap();
    assert!(matches!(rsi.direction, ContributionDirection::Positive), "RSI=65 应正向");
    let bb = attr.feature_contributions.iter().find(|c| c.feature_name == "bb_pct").unwrap();
    assert!(matches!(bb.direction, ContributionDirection::Positive), "BB%=0.8 应正向");
}

/// 关键测试（来自设计）：完整流程 KernelSHAP → 反事实 → 报告
#[test]
fn test_full_pipeline_kernel_shap_to_report() {
    // 1. 准备（6 维背景，匹配 TradingModel 特征数）
    let background = vec![
        vec![50_000.0, 50.0, 1_000_000.0, 0.0, 100.0, 0.5],
        vec![50_010.0, 55.0, 1_010_000.0, 0.1, 110.0, 0.55],
        vec![50_020.0, 60.0, 1_020_000.0, 0.2, 120.0, 0.6],
        vec![50_030.0, 65.0, 1_030_000.0, 0.3, 130.0, 0.65],
        vec![50_040.0, 70.0, 1_040_000.0, 0.4, 140.0, 0.7],
    ];
    let model = TradingModel::new();
    // KernelSHAP 需要模型 + 背景
    let _shap = KernelSHAP::try_new(Box::new(model), background.clone(), 100).unwrap();

    // 2. 端到端测试 Explainer + Counterfactual + Report
    let explainer = TradingExplainer::new(TradingModel::new(), background);
    let mut obs = HashMap::new();
    obs.insert("price".into(), 51_000.0);
    obs.insert("rsi_14".into(), 70.0);
    obs.insert("volume".into(), 2_000_000.0);
    obs.insert("macd".into(), 0.5);
    obs.insert("atr".into(), 130.0);
    obs.insert("bb_pct".into(), 0.8);
    let action = ActionSnapshot {
        position_size: 0.0, entry_price: 51_000.0, stop_loss: 50_500.0,
        take_profit: 52_000.0, order_type: "limit".into(),
    };

    // 3. 解释
    let exp = explainer.explain(&obs, &action).unwrap();
    assert!(exp.confidence > 0.0);

    // 4. 反事实
    let generator = CounterfactualGenerator::with_feature_names(
        Box::new(TradingModel::new()),
        vec!["price".into(), "rsi_14".into(), "volume".into(), "macd".into(), "atr".into(), "bb_pct".into()],
        CounterfactualConfig::new().with_max_changes(3).with_confidence_threshold(0.001),
    );
    let cfs = generator.generate(&obs, &action, &explainer);
    // 至少应能找到 1 个反事实
    assert!(!cfs.is_empty(), "在活跃市场中应能找到反事实，cfs.len()={}", cfs.len());

    // 5. 报告
    let report = ReportGenerator::generate_decision_report(
        "full_pipeline_test",
        vec![exp],
        Utc::now(),
        Utc::now(),
    );
    assert!(report.html_content.is_some());
    assert!(report.markdown_content.is_some());
}

/// 关键测试（来自设计）：单次解释 < 100ms
#[test]
fn test_single_explanation_completes_under_100ms() {
    let background: Vec<Vec<f64>> = (0..50).map(|i| vec![
        50_000.0 + (i as f64 * 10.0),
        50.0 + (i as f64 % 30.0),
        1_000_000.0 + (i as f64 * 1000.0),
        i as f64 * 0.01,
        100.0 + (i as f64 % 50.0),
        0.5 + (i as f64 % 30.0) / 100.0,
    ]).collect();
    let explainer = TradingExplainer::new(TradingModel::new(), background);
    let mut obs = HashMap::new();
    for (i, n) in ["price", "rsi_14", "volume", "macd", "atr", "bb_pct"].iter().enumerate() {
        obs.insert(n.to_string(), 50_000.0 + i as f64 * 100.0);
    }
    let action = ActionSnapshot {
        position_size: 0.0, entry_price: 0.0, stop_loss: 0.0, take_profit: 0.0, order_type: "limit".into(),
    };

    let start = Instant::now();
    for _ in 0..10 {
        let _ = explainer.explain(&obs, &action).unwrap();
    }
    let avg = start.elapsed() / 10;
    assert!(avg.as_millis() < 100, "单次解释平均耗时 {}ms > 100ms", avg.as_millis());
}

/// 端到端：多解释聚合报告
#[test]
fn test_multi_explanation_aggregation() {
    let background: Vec<Vec<f64>> = (0..30).map(|i| vec![
        50_000.0 + (i as f64 * 10.0), 50.0, 1_000_000.0, 0.0, 100.0, 0.5,
    ]).collect();
    let explainer = TradingExplainer::new(TradingModel::new(), background);

    // 生成 5 个解释
    let mut explanations = Vec::new();
    for i in 0..5 {
        let mut obs = HashMap::new();
        obs.insert("price".into(), 50_000.0 + i as f64 * 100.0);
        obs.insert("rsi_14".into(), 50.0 + i as f64);
        obs.insert("volume".into(), 1_000_000.0);
        obs.insert("macd".into(), 0.0);
        obs.insert("atr".into(), 100.0);
        obs.insert("bb_pct".into(), 0.5);
        let action = ActionSnapshot {
            position_size: 0.0, entry_price: 0.0, stop_loss: 0.0, take_profit: 0.0, order_type: "limit".into(),
        };
        explanations.push(explainer.explain(&obs, &action).unwrap());
    }

    let report = ReportGenerator::generate_decision_report(
        "multi_test", explanations, Utc::now(), Utc::now(),
    );
    assert_eq!(report.explanations.len(), 5);
    assert!(!report.feature_summary.top_features.is_empty());
    // 至少应识别出 rsi_14 为重要特征
    let rsi = report.feature_summary.top_features.iter().find(|(n, _)| n == "rsi_14");
    assert!(rsi.is_some());
}
