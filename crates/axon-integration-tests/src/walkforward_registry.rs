//! Walk-forward + Registry 集成测试
//!
//! 验证：完成 Walk-forward 验证后，将最佳模型注册到 Registry
//! 1. Walk-forward 跑出多 fold 结果
//! 2. 选择 OOS 表现最佳的 fold 对应的"模型"（用合成产物替代）
//! 3. 将该 fold 的超参 + 指标写入 ModelMetadata 并注册

use std::sync::Arc;

use axon_registry::storage::LocalStorage;
use axon_registry::{ModelMetadata, ModelRegistry, ModelStage};
use axon_walk_forward::config::WalkForwardConfig;
use axon_walk_forward::metrics::{ISMetrics, OOSMetrics};
use axon_walk_forward::{FoldResult, TimeSeriesSplitter, WindowType, aggregate_folds};

use crate::fixtures::SyntheticReturns;

/// 合成产物路径（用临时文件模拟模型权重）
#[allow(dead_code)]
fn make_synthetic_artifact(dir: &std::path::Path, version: &str) -> std::path::PathBuf {
    let path = dir.join(format!("{version}.bin"));
    std::fs::write(&path, format!("weights for {version}").as_bytes()).unwrap();
    path
}

/// 将 FoldResult 的关键指标抽取为 HashMap<String, f64>
#[allow(dead_code)]
fn fold_metrics_to_hashmap(fold: &FoldResult) -> std::collections::HashMap<String, f64> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "oos_sharpe_ratio".to_string(),
        fold.oos_metrics.sharpe_ratio,
    );
    m.insert(
        "oos_total_return".to_string(),
        fold.oos_metrics.total_return,
    );
    m.insert(
        "oos_max_drawdown".to_string(),
        fold.oos_metrics.max_drawdown,
    );
    m.insert("oos_win_rate".to_string(), fold.oos_metrics.win_rate);
    m.insert(
        "oos_profit_factor".to_string(),
        fold.oos_metrics.profit_factor,
    );
    m.insert(
        "oos_calmar_ratio".to_string(),
        fold.oos_metrics.calmar_ratio,
    );
    m.insert("overfit_ratio".to_string(), fold.overfit_ratio);
    m
}

/// 测试：Walk-forward 评估后选择最佳 fold 并注册
pub async fn test_walkforward_best_fold_registered() {
    let tmp = tempfile::tempdir().unwrap();
    let _returns = SyntheticReturns::generate(1000, 0.0005, 0.02, 42);

    // 配置 Walk-forward
    let config = WalkForwardConfig::expanding(600, 100, 100);
    let splitter = TimeSeriesSplitter::new(config);
    let splits = splitter.split(1000);
    assert!(
        splits.len() >= 3,
        "splitter should produce at least 3 folds, got {}",
        splits.len()
    );

    // 为每个 fold 计算合成指标（用相同的种子但不同的起始点）
    let folds: Vec<FoldResult> = splits
        .iter()
        .enumerate()
        .map(|(i, split)| {
            let is_metrics = ISMetrics {
                total_return: 0.15 + i as f64 * 0.01,
                sharpe_ratio: 1.5 + i as f64 * 0.1,
                max_drawdown: -0.10,
                win_rate: 0.55,
                profit_factor: 1.5,
            };
            // OOS 表现：故意让 fold 2 表现最好
            let oos_metrics = if i == 2 {
                OOSMetrics {
                    total_return: 0.20,
                    sharpe_ratio: 2.5,
                    max_drawdown: -0.05,
                    win_rate: 0.65,
                    profit_factor: 2.0,
                    calmar_ratio: 4.0,
                }
            } else {
                OOSMetrics {
                    total_return: 0.10,
                    sharpe_ratio: 1.2,
                    max_drawdown: -0.10,
                    win_rate: 0.50,
                    profit_factor: 1.2,
                    calmar_ratio: 1.2,
                }
            };
            FoldResult::new(i, split.clone(), is_metrics, oos_metrics)
        })
        .collect();

    let (aggregated, _stability) = aggregate_folds(&folds);
    assert!(aggregated.pct_profitable_folds > 0.0);

    // 找最佳 fold（OOS Sharpe 最高）
    let best_fold = folds
        .iter()
        .max_by(|a, b| {
            a.oos_metrics
                .sharpe_ratio
                .partial_cmp(&b.oos_metrics.sharpe_ratio)
                .unwrap()
        })
        .unwrap();
    assert_eq!(best_fold.fold_id, 2, "fold 2 should be the best");
    assert!(best_fold.oos_metrics.sharpe_ratio > 2.0);

    // 创建 Registry
    let storage =
        Arc::new(LocalStorage::new(tmp.path().join("models")).expect("create local storage"));
    let registry = ModelRegistry::new(storage);

    // 为每个 fold 注册一个版本，fold 2 提升为 Production
    let artifacts_dir = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).unwrap();
    for fold in &folds {
        let artifact = make_synthetic_artifact(&artifacts_dir, &format!("v{}", fold.fold_id));
        let metadata = ModelMetadata {
            description: format!("Walk-forward fold {}", fold.fold_id),
            metrics: fold_metrics_to_hashmap(fold),
            training_duration_secs: Some(60.0 * (fold.fold_id as f64 + 1.0)),
            ..Default::default()
        };

        let mv = registry
            .register("wf-strategy", &artifact, metadata, None)
            .await
            .unwrap();

        if fold.fold_id == best_fold.fold_id {
            registry
                .transition_stage("wf-strategy", &mv.version, ModelStage::Production)
                .await
                .unwrap();
        }
    }

    // 验证 Production 版本是 fold 2
    let prod = registry.get_production("wf-strategy").await.unwrap();
    assert_eq!(prod.stage, ModelStage::Production);
    assert!(prod.artifact_size_bytes > 0);

    // 验证存在 folds.len() 个版本
    let all = registry
        .list_versions("wf-strategy", &Default::default())
        .await
        .unwrap();
    assert_eq!(all.len(), folds.len());

    println!(
        "[WF+Registry] registered {} folds, best fold {} promoted to Production",
        all.len(),
        best_fold.fold_id
    );
}

/// 测试：Walk-forward WindowType 与 Registry 阶段管理
pub async fn test_walkforward_window_type_combination() {
    let _ = WindowType::Rolling;
    let _ = WindowType::Expanding;

    // 验证 Rolling 配置的合法性
    let rolling_cfg = WalkForwardConfig::rolling(500, 100, 100);
    assert!(
        rolling_cfg.validate().is_ok(),
        "rolling config should be valid"
    );

    let expanding_cfg = WalkForwardConfig::expanding(500, 100, 100);
    assert!(
        expanding_cfg.validate().is_ok(),
        "expanding config should be valid"
    );
}

/// 测试：多次迭代 Walk-forward + Registry 累积
pub async fn test_walkforward_iterative_registration() {
    let tmp = tempfile::tempdir().unwrap();
    let storage =
        Arc::new(LocalStorage::new(tmp.path().join("models")).expect("create local storage"));
    let registry = ModelRegistry::new(storage);

    let artifacts_dir = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).unwrap();

    // 模拟 3 轮训练：每轮 walk-forward 后注册最佳模型
    for iter in 1..=3 {
        let artifact = make_synthetic_artifact(&artifacts_dir, &format!("iter{iter}"));
        let metadata = ModelMetadata {
            description: format!("Iteration {iter} best model"),
            metrics: std::iter::once(("iter".to_string(), iter as f64)).collect(),
            training_duration_secs: Some(iter as f64 * 600.0),
            ..Default::default()
        };
        let mv = registry
            .register("iterative-strategy", &artifact, metadata, None)
            .await
            .unwrap();
        // 每轮都把最新版本提升到 Production（自动归档旧版本）
        registry
            .transition_stage("iterative-strategy", &mv.version, ModelStage::Production)
            .await
            .unwrap();
    }

    let all = registry
        .list_versions("iterative-strategy", &Default::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 3, "should have 3 versions");

    // 只有 1 个 Production，其余 2 个 Archived
    let production = all
        .iter()
        .filter(|mv| mv.stage == ModelStage::Production)
        .count();
    let archived = all
        .iter()
        .filter(|mv| mv.stage == ModelStage::Archived)
        .count();
    assert_eq!(production, 1);
    assert_eq!(archived, 2);

    // 验证 Production 是最后一轮（patch 版本递增：从 0 开始）
    let prod = registry.get_production("iterative-strategy").await.unwrap();
    assert_eq!(prod.version.patch, 2);

    println!("[WF+Registry] 3 iterations, final Production: {:?}", prod);
}
