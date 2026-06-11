//! HPO 多目标 + Pareto + Tracker 集成测试
//!
//! 验证：
//! 1. 多目标 HPO 评估的指标同时记录到 tracker
//! 2. Pareto 前沿计算结果与 tracker 记录的指标一致
//! 3. 选择 Pareto 最优解并注册到 registry

use std::collections::HashMap;
use std::sync::Arc;

use axon_hpo::config::{
    HPOConfig, HPORunConfig, ObjectiveConfig, ObjectiveDef, SamplerConfig, SamplerType,
    StudyConfig, StudyDirection,
};
use axon_hpo::pareto::{compute_hypervolume, compute_pareto_front, dominates};
use axon_hpo::trial::TrialResult;
use axon_registry::storage::LocalStorage;
use axon_registry::{ModelMetadata, ModelRegistry, ModelStage};
use axon_tracker::backends::MemoryTracker;
use axon_tracker::types::ParamValue;
use axon_tracker::ExperimentTracker;

use crate::fixtures::make_trial;

/// 提取 MetricEntry::Scalar 的 f64
#[allow(dead_code)]
fn scalar(m: &axon_tracker::types::MetricEntry) -> f64 {
    match &m.value {
        axon_tracker::types::MetricValue::Scalar(v) => *v,
        _ => panic!("expected Scalar metric value, got {:?}", m.value),
    }
}

/// 合成多目标评估函数：最大化 sharpe，最小化 max_drawdown
fn multi_objective_eval(params: &[(String, f64)]) -> Vec<f64> {
    let lr = params
        .iter()
        .find(|(k, _)| k == "lr")
        .map(|(_, v)| *v)
        .unwrap_or(0.001);
    let gamma = params
        .iter()
        .find(|(k, _)| k == "gamma")
        .map(|(_, v)| *v)
        .unwrap_or(0.99);

    // 目标 1：sharpe（最大化）— 在 (lr=0.001, gamma=0.99) 附近最大
    let sharpe = 2.0 - ((lr - 0.001) * 1000.0).powi(2) - ((gamma - 0.99) * 100.0).powi(2);
    // 目标 2：-max_drawdown（最大化）— 在 (lr=0.0005, gamma=0.95) 附近最大
    let max_drawdown = ((lr - 0.0005) * 1000.0).powi(2) + ((gamma - 0.95) * 100.0).powi(2);
    // 取负：sharpe 越大越优；-max_drawdown 越大（接近 0）越优；统一 maximize
    vec![sharpe, -max_drawdown]
}

/// 测试：多目标 HPO + Pareto 前沿 + Tracker
pub async fn test_multi_objective_with_pareto_and_tracker() {
    let tracker = MemoryTracker::new();

    // 合成 6 个 trial 的多目标结果
    let trial_params = vec![
        vec![("lr".to_string(), 0.001), ("gamma".to_string(), 0.99)],  // 优
        vec![("lr".to_string(), 0.0005), ("gamma".to_string(), 0.95)], // 优
        vec![("lr".to_string(), 0.002), ("gamma".to_string(), 0.90)],  // 差
        vec![("lr".to_string(), 0.0015), ("gamma".to_string(), 0.97)], // 中
        vec![("lr".to_string(), 0.0008), ("gamma".to_string(), 0.98)], // 优
        vec![("lr".to_string(), 0.003), ("gamma".to_string(), 0.85)],  // 差
    ];

    let mut trials: Vec<TrialResult> = Vec::new();

    for (i, params) in trial_params.iter().enumerate() {
        let objectives = multi_objective_eval(params);

        // 构造 trial：使用 builder 保证字段与 axon-hpo 实际类型一致
        let trial = make_trial(
            i as i32,
            params.clone(),
            objectives[0],
            axon_hpo::trial::TrialState::Complete,
        );
        // 真实试验的多目标值：替换单目标 values
        let trial = TrialResult::new(
            i as i32,
            trial.params.clone(),
            objectives.clone(),
        )
        .with_state(axon_hpo::trial::TrialState::Complete)
        .with_duration(500);
        trials.push(trial);

        // 同步到 tracker
        for (k, v) in params {
            tracker
                .log_param(k, &ParamValue::Float(*v))
                .unwrap();
        }
        tracker
            .log_metric("objective_sharpe", objectives[0], i)
            .unwrap();
        tracker
            .log_metric("objective_max_drawdown", -objectives[1], i)
            .unwrap();
    }

    // 计算 Pareto 前沿（多目标 maximize），使用 2D 精确算法路径
    let directions = vec![StudyDirection::Maximize, StudyDirection::Maximize];
    let pareto_front = compute_pareto_front(&trials, &directions).expect("pareto");
    assert!(
        !pareto_front.is_empty(),
        "Pareto front should not be empty"
    );
    // 至少有 2 个非支配解
    assert!(
        pareto_front.len() >= 2,
        "should have at least 2 Pareto-optimal points"
    );

    // 验证支配关系：Pareto 前沿中任意两点的目标值互不支配
    for i in 0..pareto_front.points.len() {
        for j in 0..pareto_front.points.len() {
            if i == j {
                continue;
            }
            assert!(
                !dominates(
                    &pareto_front.points[i].objectives,
                    &pareto_front.points[j].objectives,
                    &directions
                ),
                "Pareto points must not dominate each other"
            );
        }
    }

    // 计算超体积（参考点：0.0, -3.0），保证覆盖 trial 的目标值范围
    let hv = compute_hypervolume(&trials, &directions, &[0.0, -3.0]).expect("hypervolume");
    assert!(hv > 0.0, "hypervolume should be positive");

    // 验证 tracker 记录与 trial 一致
    let sharpe_history = tracker.get_metrics_by_key("objective_sharpe");
    let dd_history = tracker.get_metrics_by_key("objective_max_drawdown");
    assert_eq!(sharpe_history.len(), 6);
    assert_eq!(dd_history.len(), 6);

    // 选择 Pareto 最优解（前沿中 sharpe 最高的）
    let best = pareto_front
        .points
        .iter()
        .max_by(|a, b| a.objectives[0].partial_cmp(&b.objectives[0]).unwrap())
        .unwrap();
    let best_sharpe = best.objectives[0];
    let best_dd = -best.objectives[1];
    assert!(
        best_sharpe > 1.0,
        "best Pareto point should have decent sharpe, got {best_sharpe}"
    );

    // 把最优解注册到 registry
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(
        LocalStorage::new(tmp.path().join("models")).expect("create local storage"),
    );
    let registry = ModelRegistry::new(storage);

    let artifact = tmp.path().join("pareto_best.bin");
    std::fs::write(&artifact, b"pareto optimal weights").unwrap();

    let mut metrics = HashMap::new();
    metrics.insert("sharpe".to_string(), best_sharpe);
    metrics.insert("max_drawdown".to_string(), best_dd);
    metrics.insert("hypervolume".to_string(), hv);

    let metadata = ModelMetadata {
        description: format!("Pareto-optimal: sharpe={best_sharpe:.2}, dd={best_dd:.2}"),
        hyperparameters: best.params.clone(),
        metrics,
        ..Default::default()
    };

    let mv = registry
        .register("pareto-strategy", &artifact, metadata, None)
        .await
        .unwrap();
    registry
        .transition_stage("pareto-strategy", &mv.version, ModelStage::Production)
        .await
        .unwrap();

    let prod = registry.get_production("pareto-strategy").await.unwrap();
    assert_eq!(prod.stage, ModelStage::Production);
    assert!(prod.metadata.metrics.get("hypervolume").copied().unwrap() > 0.0);

    println!(
        "[Multi-obj] {} Pareto-optimal points, HV={:.4}, best sharpe={:.3}",
        pareto_front.len(),
        hv,
        best_sharpe
    );
}

/// 测试：支配关系（dominates）的传递性
pub fn test_pareto_dominance_transitivity() {
    let dirs = vec![StudyDirection::Minimize, StudyDirection::Minimize];
    let p1 = vec![1.0, 1.0];
    let p2 = vec![2.0, 2.0];
    let p3 = vec![3.0, 3.0];
    // p1 支配 p2（更小）
    assert!(dominates(&p1, &p2, &dirs));
    assert!(!dominates(&p2, &p1, &dirs));
    // p2 支配 p3
    assert!(dominates(&p2, &p3, &dirs));
    // p1 支配 p3（传递性）
    assert!(dominates(&p1, &p3, &dirs));
}

/// 测试：HPO 配置 + 多目标设置
pub fn test_hpo_multi_objective_config() {
    let config = HPOConfig {
        study: StudyConfig {
            study_name: "multi_obj_study".to_string(),
            direction: StudyDirection::Minimize, // 主目标方向
            sampler: SamplerConfig {
                sampler_type: SamplerType::Tpe {
                    n_startup_trials: 5,
                    n_warmup_steps: 0,
                },
                seed: Some(42),
            },
            ..StudyConfig::maximize("placeholder")
        },
        objective: ObjectiveConfig {
            objective: ObjectiveDef::Multi {
                directions: vec![StudyDirection::Minimize, StudyDirection::Minimize],
            },
            timeout_seconds: None,
            resource_name: "episode_reward".to_string(),
        },
        hpo: HPORunConfig {
            n_trials: 20,
            n_jobs: 1,
            timeout_seconds: None,
            early_stopping: false,
        },
        search_space: HashMap::new(),
    };

    assert_eq!(config.objective.objective.n_directions(), 2);
    assert_eq!(config.n_trials(), 20);
}
