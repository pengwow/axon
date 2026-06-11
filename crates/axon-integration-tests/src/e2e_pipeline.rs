//! 端到端训练管线集成测试
//!
//! 完整模拟：HPO 搜索超参 → Walk-forward 验证 → Tracker 记录 → Registry 注册
//! 1. HPO 评估生成多个超参 trial
//! 2. 每个 trial 在 Walk-forward 上做时序验证
//! 3. Tracker 记录每轮的训练/验证指标
//! 4. 验证完成后，最佳模型自动注册到 Registry 并提升到 Production
//! 5. 验证整体数据流一致性

use std::collections::HashMap;
use std::sync::Arc;

use axon_hpo::config::{
    HPOConfig, HPORunConfig, ObjectiveConfig, ObjectiveDef, SamplerConfig, SamplerType,
    StudyConfig, StudyDirection,
};
use axon_registry::storage::LocalStorage;
use axon_registry::{ModelMetadata, ModelRegistry, ModelStage, SemVer};
use axon_tracker::backends::MemoryTracker;
use axon_tracker::types::{MetricValue, ParamValue};
use axon_tracker::ExperimentTracker;
use axon_walk_forward::config::WalkForwardConfig;
use axon_walk_forward::metrics::{ISMetrics, OOSMetrics, FoldResult};
use axon_walk_forward::{aggregate_folds, TimeSeriesSplitter, WindowType};

use crate::fixtures::SyntheticReturns;

/// 提取 MetricEntry::Scalar 的 f64
#[allow(dead_code)]
fn scalar(m: &axon_tracker::types::MetricEntry) -> f64 {
    match &m.value {
        MetricValue::Scalar(v) => *v,
        _ => panic!("expected Scalar metric value, got {:?}", m.value),
    }
}

/// 单次 HPO trial 的 Walk-forward 评估
#[allow(dead_code)]
fn evaluate_trial_with_walkforward(
    returns: &SyntheticReturns,
    params: &[(String, f64)],
) -> Vec<FoldResult> {
    // 用 Walk-forward 分割 1000 个数据点
    let config = WalkForwardConfig::expanding(700, 100, 100);
    let splitter = TimeSeriesSplitter::new(config);
    let splits = splitter.split(1000);

    // 训练超参影响 OOS 表现：gamma 越接近 0.95 表现越好
    let gamma = params
        .iter()
        .find(|(k, _)| k == "gamma")
        .map(|(_, v)| *v)
        .unwrap_or(0.99);
    let gamma_penalty = (gamma - 0.95).powi(2) * 10.0; // gamma 越偏离 0.95 越差

    splits
        .iter()
        .enumerate()
        .map(|(i, split)| {
            let oos_metrics = returns.simulate_strategy_oos(split.test_start, split.test_end, params);
            // 加上 gamma 的影响
            let adjusted_sharpe = oos_metrics.sharpe_ratio - gamma_penalty;
            let adjusted_oos = OOSMetrics {
                sharpe_ratio: adjusted_sharpe,
                total_return: oos_metrics.total_return * (1.0 - gamma_penalty * 0.1),
                max_drawdown: oos_metrics.max_drawdown,
                win_rate: oos_metrics.win_rate,
                profit_factor: oos_metrics.profit_factor,
                calmar_ratio: oos_metrics.calmar_ratio,
            };
            // IS 略好于 OOS
            let is_metrics = ISMetrics {
                total_return: adjusted_oos.total_return * 1.05,
                sharpe_ratio: adjusted_oos.sharpe_ratio * 1.1,
                max_drawdown: adjusted_oos.max_drawdown * 0.9,
                win_rate: adjusted_oos.win_rate,
                profit_factor: adjusted_oos.profit_factor,
            };
            FoldResult::new(i, split.clone(), is_metrics, adjusted_oos)
        })
        .collect()
}

/// 端到端：HPO + Walk-forward + Tracker + Registry
pub async fn test_end_to_end_training_pipeline() {
    let tmp = tempfile::tempdir().unwrap();
    let returns = SyntheticReturns::generate(1000, 0.0005, 0.02, 42);

    // 初始化三个核心组件
    let tracker = MemoryTracker::new();
    let storage = Arc::new(
        LocalStorage::new(tmp.path().join("models")).expect("create local storage"),
    );
    let registry = ModelRegistry::new(storage);

    // 1. HPO 配置：6 个 trial 搜索 gamma
    let _hpo_config = HPOConfig {
        study: StudyConfig {
            study_name: "e2e_ppo_search".to_string(),
            direction: StudyDirection::Maximize, // 最大化 Sharpe
            sampler: SamplerConfig {
                sampler_type: SamplerType::Random,
                seed: Some(42),
            },
            ..StudyConfig::maximize("placeholder")
        },
        objective: ObjectiveConfig {
            objective: ObjectiveDef::Single {
                direction: StudyDirection::Maximize,
            },
            timeout_seconds: None,
            resource_name: "episode_reward".to_string(),
        },
        hpo: HPORunConfig {
            n_trials: 6,
            n_jobs: 1,
            timeout_seconds: None,
            early_stopping: false,
        },
        search_space: HashMap::new(),
    };

    // 2. HPO 评估循环：每个 trial 跑 walk-forward
    let gamma_trials = [0.90, 0.92, 0.95, 0.97, 0.99, 0.999];
    let mut best_oos_sharpe = f64::NEG_INFINITY;
    let mut best_gamma = 0.0;
    let mut best_trial_idx: usize = 0;

    for (i, gamma) in gamma_trials.iter().enumerate() {
        let params = vec![("gamma".to_string(), *gamma), ("lr".to_string(), 0.001)];

        // 记录 trial 参数到 tracker
        for (k, v) in &params {
            tracker.log_param(k, &ParamValue::Float(*v)).unwrap();
        }
        tracker.log_metric("trial_id", i as f64, 0).unwrap();

        // 3. 跑 Walk-forward
        let folds = evaluate_trial_with_walkforward(&returns, &params);
        let (aggregated, stability) = aggregate_folds(&folds);

        // 记录到 tracker
        for step in 0..folds.len() {
            let fold = &folds[step];
            tracker
                .log_metric("fold_oos_sharpe", fold.oos_metrics.sharpe_ratio, step)
                .unwrap();
            tracker
                .log_metric("fold_oos_return", fold.oos_metrics.total_return, step)
                .unwrap();
            tracker
                .log_metric("fold_overfit_ratio", fold.overfit_ratio, step)
                .unwrap();
        }
        tracker
            .log_metric("mean_oos_sharpe", aggregated.mean_oos_sharpe, 0)
            .unwrap();
        tracker
            .log_metric("deflated_sharpe", stability.deflated_sharpe, 0)
            .unwrap();
        tracker
            .log_metric("pct_profitable_folds", aggregated.pct_profitable_folds, 0)
            .unwrap();

        // 4. 注册到 registry（每个 trial 一个版本）
        let artifact = tmp.path().join(format!("trial_{i}.bin"));
        std::fs::write(&artifact, format!("trial {i} weights").as_bytes()).unwrap();

        let mut metrics = HashMap::new();
        metrics.insert("oos_sharpe".to_string(), aggregated.mean_oos_sharpe);
        metrics.insert("oos_return".to_string(), aggregated.mean_oos_return);
        metrics.insert("deflated_sharpe".to_string(), stability.deflated_sharpe);

        let metadata = ModelMetadata {
            description: format!("HPO trial {i} with gamma={gamma}"),
            hyperparameters: params
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                .collect(),
            metrics,
            training_duration_secs: Some(60.0 * folds.len() as f64),
            ..Default::default()
        };

        let mv = registry
            .register("e2e-ppo", &artifact, metadata, None)
            .await
            .unwrap();

        // 跟踪最佳 trial
        if aggregated.mean_oos_sharpe > best_oos_sharpe {
            best_oos_sharpe = aggregated.mean_oos_sharpe;
            best_gamma = *gamma;
            best_trial_idx = i;
            // 提升到 Production
            registry
                .transition_stage("e2e-ppo", &mv.version, ModelStage::Production)
                .await
                .unwrap();
        }
    }

    // 5. 验证端到端数据流
    // (a) Tracker 记录了 6 个 trial 的 mean_oos_sharpe
    let sharpe_history = tracker.get_metrics_by_key("mean_oos_sharpe");
    assert_eq!(sharpe_history.len(), 6);

    // (b) Registry 注册了 6 个版本
    let all = registry
        .list_versions("e2e-ppo", &Default::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 6);

    // (c) Production 版本是 gamma=0.95 的（最优）
    let prod = registry.get_production("e2e-ppo").await.unwrap();
    assert!((best_gamma - 0.95).abs() < 0.01, "最佳 gamma 应为 0.95");
    assert_eq!(prod.stage, ModelStage::Production);
    assert!(prod.metadata.hyperparameters.contains_key("gamma"));
    let gamma_in_metadata = prod
        .metadata
        .hyperparameters
        .get("gamma")
        .and_then(|v| v.as_f64())
        .unwrap();
    assert!((gamma_in_metadata - 0.95).abs() < 0.01);

    // (d) tracker 中最佳 trial 的 mean_oos_sharpe 等于注册到 Production 的指标
    let best_recorded = &sharpe_history[best_trial_idx];
    let tracker_sharpe = scalar(best_recorded);
    let registry_sharpe = prod
        .metadata
        .metrics
        .get("oos_sharpe")
        .copied()
        .unwrap();
    assert!(
        (tracker_sharpe - registry_sharpe).abs() < 1e-9,
        "tracker 和 registry 的 sharpe 应一致: tracker={tracker_sharpe}, registry={registry_sharpe}"
    );

    println!(
        "[E2E Pipeline] best gamma={}, oos_sharpe={:.3}, {} trials, {} versions registered",
        best_gamma,
        best_oos_sharpe,
        gamma_trials.len(),
        all.len()
    );
}

/// 测试：完整训练 → 注册 → 回滚流程
pub async fn test_e2e_train_register_rollback() {
    let tmp = tempfile::tempdir().unwrap();
    let tracker = MemoryTracker::new();
    let storage = Arc::new(
        LocalStorage::new(tmp.path().join("models")).expect("create local storage"),
    );
    let registry = ModelRegistry::new(storage);

    // 训练 3 个版本，v1 表现稳定，v2 表现更好（被采用），v3 表现差（被回滚）
    let versions_spec = vec![
        ("v1", 1.5, 0.10), // 稳定
        ("v2", 2.2, 0.08), // 最佳，被采用
        ("v3", 0.8, 0.30), // 表现差，需回滚
    ];

    for (i, (name, sharpe, dd)) in versions_spec.iter().enumerate() {
        // Tracker 记录
        for step in 0..50 {
            tracker
                .log_metric("loss", 1.0 / (step + 1) as f64, i * 50 + step)
                .unwrap();
            tracker
                .log_metric(
                    "val_sharpe",
                    *sharpe * step as f64 / 50.0,
                    i * 50 + step,
                )
                .unwrap();
        }
        tracker
            .log_metric("final_sharpe", *sharpe, i)
            .unwrap();
        tracker
            .log_metric("final_max_dd", *dd, i)
            .unwrap();

        // Registry 注册
        let artifact = tmp.path().join(format!("{name}.bin"));
        std::fs::write(&artifact, format!("{name} weights").as_bytes()).unwrap();

        let mut metrics = HashMap::new();
        metrics.insert("final_sharpe".to_string(), *sharpe);
        metrics.insert("final_max_dd".to_string(), *dd);

        let metadata = ModelMetadata {
            description: format!("{name}: sharpe={sharpe}, dd={dd}"),
            metrics,
            ..Default::default()
        };

        let mv = registry
            .register("deploy", &artifact, metadata, None)
            .await
            .unwrap();
        // 都先提升到 Production（自动归档旧的）
        registry
            .transition_stage("deploy", &mv.version, ModelStage::Production)
            .await
            .unwrap();
    }

    // 此时 v3 是 Production，v1 和 v2 都被归档（patch 从 0 开始递增）
    let before = registry.get_production("deploy").await.unwrap();
    assert_eq!(before.version.patch, 2, "v3 should be Production");

    // 由于 v3 表现差（sharpe=0.8），执行回滚
    let rolled_back = registry.rollback("deploy").await.unwrap();
    // 应该是 v2（被归档的最大版本）
    assert_eq!(rolled_back.version.patch, 1, "rollback should restore v2");
    assert_eq!(rolled_back.stage, ModelStage::Production);

    // 验证 v3 现在是 RolledBack
    let v3_after = registry
        .get("deploy", Some(&SemVer::new(1, 0, 2)))
        .await
        .unwrap();
    assert_eq!(v3_after.stage, ModelStage::RolledBack);

    // 验证 tracker 数据
    let sharpes = tracker.get_metrics_by_key("final_sharpe");
    assert_eq!(sharpes.len(), 3);
    assert!((scalar(&sharpes[2]) - 0.8).abs() < 1e-9);

    println!("[E2E Rollback] v3 sharpe=0.8 too low, rolled back to v2 sharpe=2.2");
}

/// 测试：跨组件的 WindowType 选择 + Tracker 报告
pub async fn test_window_type_with_tracker_reporting() {
    let tmp = tempfile::tempdir().unwrap();
    let tracker = MemoryTracker::new();
    let storage = Arc::new(
        LocalStorage::new(tmp.path().join("models")).expect("create local storage"),
    );
    let registry = ModelRegistry::new(storage);

    for (variant_idx, window_type) in [WindowType::Rolling, WindowType::Expanding]
        .iter()
        .enumerate()
    {
        let config = match window_type {
            WindowType::Rolling => WalkForwardConfig::rolling(500, 100, 100),
            WindowType::Expanding => WalkForwardConfig::expanding(500, 100, 100),
        };
        let splitter = TimeSeriesSplitter::new(config.clone());
        let n_folds = splitter.split(800).len();
        assert!(n_folds >= 2);

        // Tracker 记录
        let key = format!("{window_type:?}_n_folds");
        tracker
            .log_metric(&key, n_folds as f64, variant_idx)
            .unwrap();

        // 注册
        let artifact = tmp.path().join(format!("{window_type:?}.bin"));
        std::fs::write(&artifact, format!("{window_type:?} cfg").as_bytes()).unwrap();
        let metadata = ModelMetadata {
            description: format!("{window_type:?} walk-forward"),
            ..Default::default()
        };
        let mv = registry
            .register("window-test", &artifact, metadata, None)
            .await
            .unwrap();
        registry
            .transition_stage("window-test", &mv.version, ModelStage::Archived)
            .await
            .unwrap();
    }

    let versions = registry
        .list_versions("window-test", &Default::default())
        .await
        .unwrap();
    assert_eq!(versions.len(), 2);
    // 验证 tracker 记录了 fold 数量
    let rolling_folds = tracker.get_metrics_by_key("Rolling_n_folds");
    let expanding_folds = tracker.get_metrics_by_key("Expanding_n_folds");
    assert_eq!(rolling_folds.len(), 1);
    assert_eq!(expanding_folds.len(), 1);
}
