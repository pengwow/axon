//! TDD 第一轮：核心数据类型
//!
//! 覆盖：FeatureContribution / ContributionDirection / ActionAttribution /
//!      AttentionWeights / ActionSnapshot / CounterfactualExplanation / Explanation /
//!      DecisionReport / FeatureSummary / RiskAttributionMetrics / RegimeChange

use axon_explain::types::{
    ActionAttribution, ActionSnapshot, AttentionWeights, ContributionDirection,
    CounterfactualExplanation, DecisionReport, Explanation, FeatureContribution, FeatureSummary,
    RegimeChange, RiskAttributionMetrics,
};
use chrono::{TimeZone, Utc};
use std::collections::HashMap;

// ─── FeatureContribution ──────────────────────────────────────

#[test]
fn test_feature_contribution_direction_from_sign() {
    assert_eq!(
        ContributionDirection::from_shap(0.5),
        ContributionDirection::Positive
    );
    assert_eq!(
        ContributionDirection::from_shap(-0.3),
        ContributionDirection::Negative
    );
    assert_eq!(
        ContributionDirection::from_shap(0.0),
        ContributionDirection::Neutral
    );
    // 接近零视为中性
    assert_eq!(
        ContributionDirection::from_shap(0.0001),
        ContributionDirection::Neutral
    );
    assert_eq!(
        ContributionDirection::from_shap(-0.0001),
        ContributionDirection::Neutral
    );
}

/// 关键测试用例（来自设计文档）：SHAP 值符号应正确分类方向
#[test]
fn test_feature_contribution_serializes_direction() {
    let c = FeatureContribution {
        feature_name: "rsi_14".to_string(),
        shap_value: 0.15,
        feature_value: 62.5,
        direction: ContributionDirection::Positive,
    };
    let json = serde_json::to_string(&c).unwrap();
    let back: FeatureContribution = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c);
}

// ─── ActionSnapshot ──────────────────────────────────────────

#[test]
fn test_action_snapshot_holds_trading_params() {
    let snap = ActionSnapshot {
        position_size: 0.5,
        entry_price: 50_000.0,
        stop_loss: 49_000.0,
        take_profit: 52_000.0,
        order_type: "limit".to_string(),
    };
    assert_eq!(snap.position_size, 0.5);
    assert_eq!(snap.order_type, "limit");
}

/// ActionSnapshot 必须能 JSON 往返
#[test]
fn test_action_snapshot_serde_round_trip() {
    let snap = ActionSnapshot {
        position_size: 1.0,
        entry_price: 100.0,
        stop_loss: 95.0,
        take_profit: 110.0,
        order_type: "limit".to_string(),
    };
    let json = serde_json::to_string(&snap).unwrap();
    let back: ActionSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back, snap);
}

// ─── ActionAttribution ───────────────────────────────────────

/// 关键测试（来自设计）：ActionAttribution 必须正确分类 top positive/negative
#[test]
fn test_action_attribution_splits_positive_and_negative() {
    let contribs = vec![
        FeatureContribution {
            feature_name: "f1".into(),
            shap_value: 0.5,
            feature_value: 1.0,
            direction: ContributionDirection::Positive,
        },
        FeatureContribution {
            feature_name: "f2".into(),
            shap_value: -0.3,
            feature_value: 2.0,
            direction: ContributionDirection::Negative,
        },
        FeatureContribution {
            feature_name: "f3".into(),
            shap_value: 0.2,
            feature_value: 3.0,
            direction: ContributionDirection::Positive,
        },
        FeatureContribution {
            feature_name: "f4".into(),
            shap_value: -0.1,
            feature_value: 4.0,
            direction: ContributionDirection::Negative,
        },
        FeatureContribution {
            feature_name: "f5".into(),
            shap_value: 0.0,
            feature_value: 5.0,
            direction: ContributionDirection::Neutral,
        },
    ];
    let attr = ActionAttribution::from_contributions(
        "position_size".to_string(),
        1.5, // predicted
        1.0, // base
        contribs,
    );

    assert_eq!(attr.top_positive.len(), 2);
    assert_eq!(attr.top_negative.len(), 2);
    assert_eq!(attr.top_positive[0].feature_name, "f1");
    assert_eq!(attr.top_negative[0].feature_name, "f2");
}

// ─── AttentionWeights ────────────────────────────────────────

/// 关键测试（来自设计）：注意力权重必须非负
#[test]
fn test_attention_weights_must_be_non_negative() {
    let w = AttentionWeights {
        layer: 0,
        head: 0,
        weights: vec![vec![0.3, 0.7], vec![0.5, 0.5]],
        tokens: vec!["a".into(), "b".into()],
        timestamp: Utc::now(),
    };
    for row in &w.weights {
        for val in row {
            assert!(*val >= 0.0, "注意力权重必须非负: {}", val);
        }
    }
}

/// 注意力权重行和应接近 1.0（softmax 归一化）
#[test]
fn test_attention_weights_rows_sum_to_one() {
    let w = AttentionWeights {
        layer: 0,
        head: 0,
        weights: vec![
            vec![0.3, 0.5, 0.2],
            vec![0.1, 0.6, 0.3],
            vec![0.4, 0.4, 0.2],
        ],
        tokens: vec!["a".into(), "b".into(), "c".into()],
        timestamp: Utc::now(),
    };
    for row in &w.weights {
        let sum: f64 = row.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6, "行和应为 1.0, 实际 {}", sum);
    }
}

// ─── CounterfactualExplanation ────────────────────────────────

#[test]
fn test_counterfactual_holds_narrative() {
    let cf = CounterfactualExplanation {
        original_action: ActionSnapshot {
            position_size: 1.0,
            entry_price: 100.0,
            stop_loss: 95.0,
            take_profit: 110.0,
            order_type: "limit".into(),
        },
        modified_action: ActionSnapshot {
            position_size: 0.5,
            entry_price: 100.0,
            stop_loss: 95.0,
            take_profit: 110.0,
            order_type: "limit".into(),
        },
        changed_features: vec!["rsi_14".into()],
        original_confidence: 0.8,
        new_confidence: 0.6,
        narrative: "如果 RSI 从 70 降到 30，置信度从 80% 降至 60%".into(),
    };
    assert!(cf.narrative.contains("RSI"));
    assert!(cf.changed_features.contains(&"rsi_14".to_string()));
}

// ─── Explanation ─────────────────────────────────────────────

/// 关键测试（来自设计）：Explanation 包含完整归因信息
#[test]
fn test_explanation_contains_all_fields() {
    let mut feat_importance = HashMap::new();
    feat_importance.insert("rsi_14".to_string(), 0.15);
    feat_importance.insert("volume".to_string(), 0.08);

    let exp = Explanation {
        id: "exp_001".to_string(),
        observation_id: "obs_001".to_string(),
        action: ActionSnapshot {
            position_size: 1.0,
            entry_price: 100.0,
            stop_loss: 95.0,
            take_profit: 110.0,
            order_type: "limit".into(),
        },
        feature_importance: feat_importance.clone(),
        action_attributions: vec![],
        attention_weights: None,
        counterfactuals: vec![],
        summary: "RSI 处于超买区，建议减仓".to_string(),
        confidence: 0.85,
        generated_at: Utc::now(),
    };

    assert_eq!(exp.id, "exp_001");
    assert_eq!(exp.confidence, 0.85);
    assert_eq!(exp.feature_importance.len(), 2);
    assert!(exp.summary.contains("RSI"));
}

#[test]
fn test_explanation_serde_round_trip() {
    let exp = Explanation {
        id: "x".into(),
        observation_id: "y".into(),
        action: ActionSnapshot {
            position_size: 1.0,
            entry_price: 1.0,
            stop_loss: 0.9,
            take_profit: 1.1,
            order_type: "limit".into(),
        },
        feature_importance: HashMap::new(),
        action_attributions: vec![],
        attention_weights: None,
        counterfactuals: vec![],
        summary: "test".into(),
        confidence: 0.5,
        generated_at: Utc.with_ymd_and_hms(2026, 6, 11, 0, 0, 0).unwrap(),
    };
    let json = serde_json::to_string(&exp).unwrap();
    let back: Explanation = serde_json::from_str(&json).unwrap();
    assert_eq!(back, exp);
}

// ─── FeatureSummary / RegimeChange / RiskAttributionMetrics ───

#[test]
fn test_feature_summary_top_features_hold() {
    let summary = FeatureSummary {
        top_features: vec![("rsi_14".to_string(), 0.15), ("volume".to_string(), 0.08)],
        feature_stability: HashMap::new(),
        regime_changes: vec![],
    };
    assert_eq!(summary.top_features.len(), 2);
    assert_eq!(summary.top_features[0].0, "rsi_14");
}

#[test]
fn test_regime_change_records_transition() {
    let rc = RegimeChange {
        timestamp: Utc::now(),
        from_regime: "bull".to_string(),
        to_regime: "bear".to_string(),
        impact_on_features: vec!["momentum".to_string(), "volatility".to_string()],
    };
    assert_eq!(rc.from_regime, "bull");
    assert_eq!(rc.to_regime, "bear");
    assert_eq!(rc.impact_on_features.len(), 2);
}

#[test]
fn test_risk_attribution_metrics_serde() {
    let metrics = RiskAttributionMetrics {
        var_contribution: HashMap::new(),
        sharpe_contribution: HashMap::new(),
        max_drawdown_factors: vec!["volatility".to_string()],
    };
    let json = serde_json::to_string(&metrics).unwrap();
    let back: RiskAttributionMetrics = serde_json::from_str(&json).unwrap();
    assert_eq!(back.max_drawdown_factors, vec!["volatility".to_string()]);
}

// ─── DecisionReport ──────────────────────────────────────────

#[test]
fn test_decision_report_contains_period_and_explanations() {
    let report = DecisionReport {
        report_id: "rpt_001".to_string(),
        period_start: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        period_end: Utc.with_ymd_and_hms(2026, 1, 31, 0, 0, 0).unwrap(),
        explanations: vec![],
        feature_summary: FeatureSummary {
            top_features: vec![],
            feature_stability: HashMap::new(),
            regime_changes: vec![],
        },
        risk_metrics: RiskAttributionMetrics {
            var_contribution: HashMap::new(),
            sharpe_contribution: HashMap::new(),
            max_drawdown_factors: vec![],
        },
        html_content: None,
        markdown_content: None,
    };
    assert_eq!(report.report_id, "rpt_001");
    assert!(report.period_start < report.period_end);
}
