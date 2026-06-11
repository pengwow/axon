//! 错误恢复与并发场景的集成测试
//!
//! 覆盖横向任务中的集成测试：
//! 1. HPO trial 失败时 Tracker 记录失败状态，不污染 Registry
//! 2. 多线程并发注册到同一 model 的线程安全
//! 3. 跨组件数据一致性：tracker 记录 → registry 验证 → 一致性校验
//! 4. Walk-forward 配合 purge 函数（防泄漏场景）

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use axon_hpo::config::{
    HPOConfig, HPORunConfig, ObjectiveConfig, ObjectiveDef, SamplerConfig, SamplerType,
    StudyConfig, StudyDirection,
};
use axon_hpo::trial::TrialState;
use axon_registry::storage::LocalStorage;
use axon_registry::{ModelMetadata, ModelRegistry, ModelStage, SemVer};
use axon_tracker::backends::MemoryTracker;
use axon_tracker::types::{MetricValue, ParamValue};
use axon_tracker::ExperimentTracker;
use axon_walk_forward::config::WalkForwardConfig;
use axon_walk_forward::metrics::{ISMetrics, OOSMetrics, FoldResult};
use axon_walk_forward::purge::{detect_leakage, embargo_indices, purge_overlapping_labels};
use axon_walk_forward::{aggregate_folds, TimeSeriesSplitter, WindowType};

use crate::fixtures::SyntheticReturns;

/// 提取 MetricValue::Scalar 的 f64
fn scalar(m: &axon_tracker::types::MetricEntry) -> f64 {
    match &m.value {
        MetricValue::Scalar(v) => *v,
        _ => panic!("expected Scalar metric value, got {:?}", m.value),
    }
}

/// 场景 1：HPO trial 失败时 Tracker 记录失败状态，Registry 不被污染
///
/// 验证：
/// - 失败的 trial 仍记录在 tracker 中（state = Fail）
/// - 成功的 trial 注册到 registry
/// - registry 中没有失败 trial 的残留
pub async fn test_hpo_failure_does_not_pollute_registry() {
    let tracker = MemoryTracker::new();
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path().join("models")).unwrap());
    let registry = ModelRegistry::new(storage);

    // 5 个 trial：3 个成功、2 个失败
    let trials: Vec<(i32, f64, TrialState, bool)> = vec![
        (1, 0.5, TrialState::Complete, true),
        (2, 0.3, TrialState::Fail, false),
        (3, 0.7, TrialState::Complete, true),
        (4, 0.0, TrialState::Fail, false),
        (5, 0.4, TrialState::Complete, true),
    ];

    for (id, value, state, should_register) in &trials {
        // Tracker 记录 trial 参数和状态
        tracker.log_param("trial_id", &ParamValue::Int(*id as i64)).unwrap();
        tracker.log_param("lr", &ParamValue::Float(0.001)).unwrap();
        tracker
            .log_metric("objective", *value, *id as usize)
            .unwrap();
        // 记录 trial 状态
        let state_str = match state {
            TrialState::Complete => "complete",
            TrialState::Fail => "failed",
            _ => "other",
        };
        tracker
            .log_metric(
                "trial_state_indicator",
                if matches!(state, TrialState::Complete) { 1.0 } else { 0.0 },
                *id as usize,
            )
            .unwrap();
        tracker
            .log_param("trial_state", &ParamValue::String(state_str.to_string()))
            .unwrap();

        // 仅 Complete 状态的 trial 注册到 registry
        if *should_register {
            let artifact = tmp.path().join(format!("trial_{id}.bin"));
            std::fs::write(&artifact, format!("weights {id}").as_bytes()).unwrap();
            let mut metrics = HashMap::new();
            metrics.insert("objective".to_string(), *value);
            let metadata = ModelMetadata {
                description: format!("trial {id} objective={value}"),
                metrics,
                ..Default::default()
            };
            registry
                .register("ppo_search", &artifact, metadata, None)
                .await
                .unwrap();
        }
    }

    // 验证：tracker 记录了 5 个 trial（无论成功失败）
    let objective_history = tracker.get_metrics_by_key("objective");
    assert_eq!(objective_history.len(), 5);

    // 验证：registry 只注册了 3 个成功的 trial
    let versions = registry
        .list_versions("ppo_search", &Default::default())
        .await
        .unwrap();
    assert_eq!(versions.len(), 3, "registry 应仅含 3 个成功 trial");

    // 验证：tracker 记录的 trial_state 参数反映了实际状态
    let state_param = tracker.get_param("trial_state").unwrap();
    if let ParamValue::String(s) = state_param {
        // 最后一个 trial (id=5) 是 Complete
        assert_eq!(s, "complete");
    } else {
        panic!("trial_state 应为 String，得到 {state_param:?}");
    }

    // 验证：失败的 trial 客观值仍保留在 tracker（用于失败模式分析）
    // 失败的 trial (id=2 value=0.3, id=4 value=0.0)
    let failed_objectives: Vec<f64> = objective_history
        .iter()
        .zip(trials.iter())
        .filter_map(|(m, (_, _, state, _))| match (&m.value, state) {
            (MetricValue::Scalar(v), TrialState::Fail) => Some(*v),
            _ => None,
        })
        .collect();
    assert_eq!(failed_objectives.len(), 2, "应记录 2 个失败 trial 的 objective");
    assert!(failed_objectives.contains(&0.3), "trial 2 objective 应保留");
    assert!(failed_objectives.contains(&0.0), "trial 4 objective 应保留");

    println!(
        "[HPO Failure Recovery] 5 trials (3 ok + 2 failed), tracker: 5, registry: {}",
        versions.len()
    );
}

/// 场景 2：多线程并发注册到同一 model 的线程安全
///
/// MemoryTracker 内部使用 Mutex，所以 Arc<MemoryTracker> 可跨线程共享。
pub async fn test_concurrent_registry_registrations() {
    let tracker = Arc::new(MemoryTracker::new());
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path().join("models")).unwrap());
    let registry = Arc::new(ModelRegistry::new(storage));

    const N_THREADS: usize = 8;
    const VERSIONS_PER_THREAD: usize = 5;

    let mut handles = Vec::with_capacity(N_THREADS);
    for thread_id in 0..N_THREADS {
        let registry = Arc::clone(&registry);
        let tracker = Arc::clone(&tracker);
        let tmp_path = tmp.path().to_path_buf();
        handles.push(thread::spawn(move || {
            // 用多线程版 runtime
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                for v in 0..VERSIONS_PER_THREAD {
                    let artifact = tmp_path.join(format!("t{thread_id}_v{v}.bin"));
                    std::fs::write(
                        &artifact,
                        format!("thread {thread_id} version {v}").as_bytes(),
                    )
                    .unwrap();

                    let mut metrics = HashMap::new();
                    metrics.insert("thread_id".to_string(), thread_id as f64);
                    metrics.insert("local_version".to_string(), v as f64);

                    let metadata = ModelMetadata {
                        description: format!("concurrent t{thread_id}v{v}"),
                        metrics,
                        ..Default::default()
                    };

                    let mv = registry
                        .register("concurrent_model", &artifact, metadata, None)
                        .await
                        .unwrap();

                    tracker
                        .log_metric(
                            "thread_version",
                            (thread_id * VERSIONS_PER_THREAD + v) as f64,
                            thread_id,
                        )
                        .unwrap();

                    // 抑制 unused 警告
                    let _ = mv;
                }
            });
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }

    // 验证：共注册 N_THREADS * VERSIONS_PER_THREAD = 40 个版本
    let all = registry
        .list_versions("concurrent_model", &Default::default())
        .await
        .unwrap();
    assert_eq!(
        all.len(),
        N_THREADS * VERSIONS_PER_THREAD,
        "应注册 {} 个版本，实际 {} 个",
        N_THREADS * VERSIONS_PER_THREAD,
        all.len()
    );

    // 验证：所有版本号唯一
    let mut versions: Vec<SemVer> = all.iter().map(|mv| mv.version.clone()).collect();
    versions.sort_by(|a, b| a.patch.cmp(&b.patch));
    for w in versions.windows(2) {
        assert_ne!(w[0], w[1], "版本号重复: {}", w[0]);
    }

    // 验证：可以查询任意版本
    let last_version = versions.last().unwrap();
    let retrieved = registry
        .get("concurrent_model", Some(last_version))
        .await
        .unwrap();
    assert_eq!(retrieved.version, *last_version);

    // 验证：tracker 跨线程记录了所有 trial（MemoryTracker Mutex 保证安全）
    let thread_versions = tracker.get_metrics_by_key("thread_version");
    assert_eq!(thread_versions.len(), N_THREADS * VERSIONS_PER_THREAD);

    println!(
        "[Concurrent Registry] {} threads × {} versions = {} registered, all unique",
        N_THREADS,
        VERSIONS_PER_THREAD,
        all.len()
    );
}

/// 场景 3：跨组件数据一致性
///
/// Tracker 记录的指标应该等于 Registry metadata 中的 metrics，
/// 避免"两份真相"导致的运维错误。
pub async fn test_tracker_registry_data_consistency() {
    let tracker = MemoryTracker::new();
    let tmp = tempfile::tempdir().unwrap();
    let storage = Arc::new(LocalStorage::new(tmp.path().join("models")).unwrap());
    let registry = ModelRegistry::new(storage);

    // 模拟训练 4 个 epoch，每 epoch 记录 loss + val_sharpe
    let n_epochs = 4;
    let final_sharpe = 2.3;
    let final_loss = 0.05;

    for epoch in 0..n_epochs {
        tracker
            .log_metric("loss", 1.0 / (epoch + 1) as f64, epoch)
            .unwrap();
        tracker
            .log_metric("val_sharpe", 0.5 + epoch as f64 * 0.5, epoch)
            .unwrap();
    }
    // 记录最终指标
    tracker.log_metric("final_loss", final_loss, 0).unwrap();
    tracker.log_metric("final_sharpe", final_sharpe, 0).unwrap();

    // 从 tracker 读取最终指标，构建 metadata
    let recorded_final_sharpe = scalar(&tracker.get_metrics_by_key("final_sharpe")[0]);
    let recorded_final_loss = scalar(&tracker.get_metrics_by_key("final_loss")[0]);
    assert!((recorded_final_sharpe - final_sharpe).abs() < 1e-9);
    assert!((recorded_final_loss - final_loss).abs() < 1e-9);

    // 注册到 registry
    let artifact = tmp.path().join("consistent_model.bin");
    std::fs::write(&artifact, b"consistent weights").unwrap();

    let mut metrics = HashMap::new();
    metrics.insert("final_sharpe".to_string(), recorded_final_sharpe);
    metrics.insert("final_loss".to_string(), recorded_final_loss);
    metrics.insert("n_epochs".to_string(), n_epochs as f64);

    let metadata = ModelMetadata {
        description: "data consistency test model".to_string(),
        metrics: metrics.clone(),
        ..Default::default()
    };
    let mv = registry
        .register("consistent_model", &artifact, metadata, None)
        .await
        .unwrap();

    // 验证：registry 中查到的 metrics 与 tracker 中读取的一致
    let registered = registry
        .get("consistent_model", Some(&mv.version))
        .await
        .unwrap();
    let reg_sharpe = registered.metadata.metrics.get("final_sharpe").copied().unwrap();
    let reg_loss = registered.metadata.metrics.get("final_loss").copied().unwrap();
    let reg_epochs = registered.metadata.metrics.get("n_epochs").copied().unwrap();

    assert!((reg_sharpe - recorded_final_sharpe).abs() < 1e-9);
    assert!((reg_loss - recorded_final_loss).abs() < 1e-9);
    assert!((reg_epochs - n_epochs as f64).abs() < 1e-9);

    println!(
        "[Data Consistency] tracker ↔ registry sharpe={:.3}, loss={:.3}, epochs={}",
        reg_sharpe, reg_loss, reg_epochs
    );
}

/// 场景 4：Walk-forward + purge 函数（防泄漏）后注册
///
/// 验证 purge + embargo 函数能正确处理时间序列训练/测试索引的分离，
/// 防止 label_horizon 范围内的特征泄漏。
pub async fn test_purged_walkforward_registration() {
    let tmp = tempfile::tempdir().unwrap();
    let tracker = MemoryTracker::new();
    let storage = Arc::new(LocalStorage::new(tmp.path().join("models")).unwrap());
    let registry = ModelRegistry::new(storage);

    // 1. 用标准 splitter 生成 splits
    let returns = SyntheticReturns::generate(2000, 0.0005, 0.02, 42);
    let config = WalkForwardConfig::rolling(500, 100, 100);
    let splitter = TimeSeriesSplitter::new(config.clone());
    let splits = splitter.split(2000);
    assert!(!splits.is_empty());

    // 2. 验证第一个 fold 的 train/test 索引分离正确
    let first_split = &splits[0];
    let train_idx: Vec<usize> = (first_split.train_start..first_split.train_end).collect();
    let test_idx: Vec<usize> = (first_split.test_start..first_split.test_end).collect();

    // 标准检测：train/test 严格分离（无重叠）
    let (has_leakage, leaked) = detect_leakage(&train_idx, &test_idx, 0);
    assert!(!has_leakage, "无 label_horizon 时不应有泄漏");
    assert!(leaked.is_empty(), "泄漏索引应为空");

    // 3. 用 purge_overlapping_labels 处理（label_horizon = 10）
    let label_horizon = 10;
    let purged_train = purge_overlapping_labels(&train_idx, &test_idx, label_horizon);
    assert!(
        purged_train.len() < train_idx.len(),
        "purge 后训练索引应减少：原 {} → 清洗后 {}",
        train_idx.len(),
        purged_train.len()
    );

    // 4. 用 embargo_indices 在测试后加隔离期
    let n_total = 2000;
    let embargoed = embargo_indices(&test_idx, 0.05, n_total);
    assert!(!embargoed.is_empty(), "embargo 索引应非空");

    // 5. 跑评估
    let mut all_results: Vec<(usize, f64)> = Vec::new();
    for (orig_idx, split) in splits.iter().enumerate() {
        let oos = returns.simulate_strategy_oos(
            split.test_start,
            split.test_end,
            &[("gamma".to_string(), 0.99)],
        );
        all_results.push((orig_idx, oos.sharpe_ratio));
    }

    // 记录到 tracker
    for (i, (_, sharpe)) in all_results.iter().enumerate() {
        tracker.log_metric("purged_oos_sharpe", *sharpe, i).unwrap();
    }

    // 6. 注册"防泄漏验证后"的模型
    let best_sharpe = all_results
        .iter()
        .map(|(_, s)| *s)
        .fold(f64::NEG_INFINITY, f64::max);

    let artifact = tmp.path().join("purged_model.bin");
    std::fs::write(&artifact, b"purged walk-forward model").unwrap();
    let mut metrics = HashMap::new();
    metrics.insert("purged_oos_sharpe".to_string(), best_sharpe);
    metrics.insert("label_horizon".to_string(), label_horizon as f64);
    metrics.insert("embargo_pct".to_string(), 0.05);
    let metadata = ModelMetadata {
        description: "Walk-forward + purge + embargo 防泄漏验证后注册".to_string(),
        metrics,
        ..Default::default()
    };
    let mv = registry
        .register("purged_model", &artifact, metadata, None)
        .await
        .unwrap();
    registry
        .transition_stage("purged_model", &mv.version, ModelStage::Production)
        .await
        .unwrap();

    // 验证
    let prod = registry.get_production("purged_model").await.unwrap();
    assert_eq!(prod.stage, ModelStage::Production);
    let registered_sharpe = prod
        .metadata
        .metrics
        .get("purged_oos_sharpe")
        .copied()
        .unwrap();
    assert!((registered_sharpe - best_sharpe).abs() < 1e-9);

    // 验证 tracker 记录了 purged_oos_sharpe（无需断言 last=best，只需存在性）
    let purged_sharpes = tracker.get_metrics_by_key("purged_oos_sharpe");
    assert!(!purged_sharpes.is_empty());
    let recorded_max = purged_sharpes
        .iter()
        .filter_map(|m| match &m.value {
            MetricValue::Scalar(v) => Some(*v),
            _ => None,
        })
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (recorded_max - best_sharpe).abs() < 1e-9,
        "tracker 中 max={:.3} 应等于 best={:.3}",
        recorded_max,
        best_sharpe
    );

    println!(
        "[Purged Walk-forward] {} splits → label_horizon={}, embargo=5% → best sharpe={:.3}",
        splits.len(),
        label_horizon,
        best_sharpe
    );
}

/// 场景 5：HPO + Walk-forward 配置 TOML/JSON 序列化往返
pub fn test_config_serialization_roundtrip() {
    // 构造 HPO 配置（Tpe 是 struct 变体）
    let hpo_config = HPOConfig {
        study: StudyConfig {
            study_name: "roundtrip_test".to_string(),
            direction: StudyDirection::Maximize,
            sampler: SamplerConfig {
                sampler_type: SamplerType::Tpe {
                    n_startup_trials: 10,
                    n_warmup_steps: 5,
                },
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
            n_trials: 20,
            n_jobs: 2,
            timeout_seconds: Some(3600),
            early_stopping: true,
        },
        search_space: HashMap::new(),
    };

    // 序列化 → 反序列化
    let json = serde_json::to_string(&hpo_config).expect("serialize HPO config");
    let restored: HPOConfig = serde_json::from_str(&json).expect("deserialize HPO config");
    assert_eq!(restored.n_trials(), 20);
    assert_eq!(restored.study.study_name, "roundtrip_test");
    assert_eq!(restored.hpo.n_jobs, 2);

    // 验证 Walk-forward 配置序列化
    let wf_config = WalkForwardConfig::rolling(500, 100, 50);
    let json = serde_json::to_string(&wf_config).expect("serialize WF config");
    let restored: WalkForwardConfig = serde_json::from_str(&json).expect("deserialize WF config");
    let splitter = TimeSeriesSplitter::new(restored);
    let splits = splitter.split(1000);
    assert!(!splits.is_empty());

    // 验证 WindowType 序列化
    let wt = WindowType::Expanding;
    let json = serde_json::to_string(&wt).unwrap();
    let restored: WindowType = serde_json::from_str(&json).unwrap();
    assert!(matches!(restored, WindowType::Expanding));

    // 验证 TrialState 序列化（用于持久化失败 trial）
    let states = vec![TrialState::Complete, TrialState::Fail, TrialState::Pruned];
    for state in states {
        let json = serde_json::to_string(&state).unwrap();
        let _: TrialState = serde_json::from_str(&json).unwrap();
    }

    println!("[Config Roundtrip] HPO + WF + WindowType + TrialState 序列化/反序列化均通过");
}

/// 场景 6：聚合 OOS 指标后注册到 Registry
pub async fn test_aggregate_oos_then_register() {
    let tmp = tempfile::tempdir().unwrap();
    let tracker = MemoryTracker::new();
    let storage = Arc::new(LocalStorage::new(tmp.path().join("models")).unwrap());
    let registry = ModelRegistry::new(storage);

    let returns = SyntheticReturns::generate(1500, 0.0005, 0.02, 7);
    let config = WalkForwardConfig::expanding(500, 100, 100);
    let splitter = TimeSeriesSplitter::new(config);
    let splits = splitter.split(1500);
    assert!(splits.len() >= 3);

    // 模拟策略评估：用 fold 索引影响表现
    let folds: Vec<FoldResult> = splits
        .iter()
        .enumerate()
        .map(|(i, split)| {
            let oos = returns.simulate_strategy_oos(
                split.test_start,
                split.test_end,
                &[("trial_idx".to_string(), i as f64)],
            );
            let is = ISMetrics {
                total_return: oos.total_return * 1.1,
                sharpe_ratio: oos.sharpe_ratio * 1.15,
                max_drawdown: oos.max_drawdown,
                win_rate: oos.win_rate,
                profit_factor: oos.profit_factor,
            };
            FoldResult::new(i, split.clone(), is, oos)
        })
        .collect();

    let (aggregated, stability) = aggregate_folds(&folds);

    // 记录聚合指标
    tracker
        .log_metric("mean_oos_sharpe", aggregated.mean_oos_sharpe, 0)
        .unwrap();
    tracker
        .log_metric("mean_oos_return", aggregated.mean_oos_return, 0)
        .unwrap();
    tracker
        .log_metric("deflated_sharpe", stability.deflated_sharpe, 0)
        .unwrap();
    tracker
        .log_metric("pct_profitable_folds", aggregated.pct_profitable_folds, 0)
        .unwrap();

    // 注册
    let artifact = tmp.path().join("aggregated_model.bin");
    std::fs::write(&artifact, b"aggregated model").unwrap();
    let mut metrics = HashMap::new();
    metrics.insert("mean_oos_sharpe".to_string(), aggregated.mean_oos_sharpe);
    metrics.insert("mean_oos_return".to_string(), aggregated.mean_oos_return);
    metrics.insert("deflated_sharpe".to_string(), stability.deflated_sharpe);
    metrics.insert("pct_profitable_folds".to_string(), aggregated.pct_profitable_folds);
    let metadata = ModelMetadata {
        description: "Aggregated OOS metrics after walk-forward".to_string(),
        metrics: metrics.clone(),
        ..Default::default()
    };
    let mv = registry
        .register("aggregated_model", &artifact, metadata, None)
        .await
        .unwrap();
    registry
        .transition_stage("aggregated_model", &mv.version, ModelStage::Production)
        .await
        .unwrap();

    // 验证
    let prod = registry.get_production("aggregated_model").await.unwrap();
    assert_eq!(prod.stage, ModelStage::Production);
    for (k, v) in &metrics {
        let reg_v = prod.metadata.metrics.get(k).copied().unwrap();
        assert!((reg_v - v).abs() < 1e-9, "metric {k}: tracker={v}, registry={reg_v}");
    }

    println!(
        "[Aggregate OOS Register] {} folds, mean sharpe={:.3}, deflated={:.3}",
        folds.len(),
        aggregated.mean_oos_sharpe,
        stability.deflated_sharpe
    );
}
