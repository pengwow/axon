//! 集成测试入口
//!
//! 启用 axon-integration-tests crate 内的所有集成测试模块

#![allow(clippy::needless_range_loop)]

use axon_integration_tests::e2e_pipeline;
use axon_integration_tests::hpo_tracker;
use axon_integration_tests::multi_objective;
use axon_integration_tests::tracker_registry;
use axon_integration_tests::walkforward_registry;

// HPO + Tracker 集成测试
#[test]
fn hpo_tracker_trial_tracking() {
    hpo_tracker::run_hpo_trial_tracking();
}

#[test]
fn hpo_tracker_config_simulation() {
    hpo_tracker::run_hpo_config_simulation();
}

#[test]
fn hpo_tracker_batch_param_logging() {
    hpo_tracker::run_hpo_batch_param_logging();
}

// Walk-forward + Registry 集成测试
#[tokio::test]
async fn walkforward_registry_basic_flow() {
    walkforward_registry::test_walkforward_best_fold_registered().await;
}

#[tokio::test]
async fn walkforward_registry_window_combination() {
    walkforward_registry::test_walkforward_window_type_combination().await;
}

#[tokio::test]
async fn walkforward_registry_iterative_registration() {
    walkforward_registry::test_walkforward_iterative_registration().await;
}

// Tracker + Registry 集成测试
#[tokio::test]
async fn tracker_registry_metrics_drive_promotion() {
    tracker_registry::test_tracker_metrics_drive_promotion().await;
}

#[tokio::test]
async fn tracker_registry_metadata_consistency() {
    tracker_registry::test_tracker_registry_metadata_consistency().await;
}

#[tokio::test]
async fn tracker_registry_flush_independence() {
    tracker_registry::test_tracker_flush_independent_from_registry().await;
}

// 多目标 HPO + Pareto + Tracker
#[tokio::test]
async fn multi_objective_pareto_tracker() {
    multi_objective::test_multi_objective_with_pareto_and_tracker().await;
}

#[test]
fn multi_objective_dominance_transitivity() {
    multi_objective::test_pareto_dominance_transitivity();
}

#[test]
fn multi_objective_hpo_config() {
    multi_objective::test_hpo_multi_objective_config();
}

// 端到端训练管线
#[tokio::test]
async fn e2e_pipeline_full() {
    e2e_pipeline::test_end_to_end_training_pipeline().await;
}

#[tokio::test]
async fn e2e_pipeline_train_register_rollback() {
    e2e_pipeline::test_e2e_train_register_rollback().await;
}

#[tokio::test]
async fn e2e_pipeline_window_type_tracker() {
    e2e_pipeline::test_window_type_with_tracker_reporting().await;
}
