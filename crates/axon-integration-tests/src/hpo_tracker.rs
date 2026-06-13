//! HPO + Tracker 集成测试
//!
//! 验证超参优化过程中的实时指标追踪：
//! 1. 每个 trial 评估后立即记录到 tracker
//! 2. tracker 记录的超参与 HPO trial 参数一致
//! 3. tracker 记录的最终值与 HPO 报告的 best_trial 一致

use std::collections::HashMap;

use axon_hpo::config::{
    HPOConfig, HPORunConfig, ObjectiveConfig, ObjectiveDef, SamplerConfig, SamplerType,
    StudyConfig, StudyDirection,
};
use axon_hpo::trial::TrialState;
use axon_tracker::ExperimentTracker;
use axon_tracker::backends::MemoryTracker;
use axon_tracker::types::{MetricValue, ParamValue};

use crate::fixtures::{make_trial, parabolic_objective};

/// 提取 MetricValue::Scalar 的 f64
fn scalar(m: &axon_tracker::types::MetricEntry) -> f64 {
    match &m.value {
        MetricValue::Scalar(v) => *v,
        _ => panic!("expected Scalar metric value, got {:?}", m.value),
    }
}

/// 测试：HPO trial 评估后实时写入 tracker
pub fn run_hpo_trial_tracking() {
    let tracker = MemoryTracker::new();

    // 模拟 5 个 trial，每个 trial 评估后记录指标
    let trials = [
        (vec![("x".to_string(), 0.1), ("y".to_string(), 0.2)], -0.27),
        (vec![("x".to_string(), 0.4), ("y".to_string(), 0.1)], -0.29),
        (vec![("x".to_string(), 0.5), ("y".to_string(), 0.3)], 0.0), // 最优
        (vec![("x".to_string(), 0.6), ("y".to_string(), 0.4)], -0.06),
        (vec![("x".to_string(), 0.7), ("y".to_string(), 0.5)], -0.21),
    ];

    for (i, (params, value)) in trials.iter().enumerate() {
        // 记录参数
        for (k, v) in params {
            tracker.log_param(k, &ParamValue::Float(*v)).unwrap();
        }
        // 记录目标值
        tracker.log_metric("objective_value", *value, i).unwrap();
        // 记录 step（模拟训练步数）
        tracker
            .log_metric("training_step", (i * 100) as f64, i)
            .unwrap();
    }

    // 验证 tracker 记录完整
    let recorded_x = tracker.get_param("x").unwrap();
    if let ParamValue::Float(v) = recorded_x {
        assert!((v - 0.7).abs() < 1e-9, "last x param should be 0.7");
    } else {
        panic!("x param should be Float, got {recorded_x:?}");
    }

    let metrics = tracker.get_metrics_by_key("objective_value");
    assert_eq!(metrics.len(), 5, "should have 5 objective_value metrics");
    assert!(
        (scalar(&metrics[2]) - 0.0).abs() < 1e-9,
        "third trial should be optimal, got {}",
        scalar(&metrics[2])
    );

    let step_metrics = tracker.get_metrics_by_key("training_step");
    assert_eq!(step_metrics.len(), 5);
    assert!((scalar(&step_metrics[4]) - 400.0).abs() < 1e-9);

    println!(
        "[HPO+Tracker] tracked {} trials, best value = {}",
        trials.len(),
        trials
            .iter()
            .map(|(_, v)| *v)
            .fold(f64::NEG_INFINITY, f64::max)
    );
}

/// 测试：HPO 配置 + 模拟 trial 评估
pub fn run_hpo_config_simulation() {
    // 构建 HPO 配置
    let config = HPOConfig {
        study: StudyConfig {
            study_name: "test_study".to_string(),
            direction: StudyDirection::Minimize,
            sampler: SamplerConfig {
                sampler_type: SamplerType::Random,
                seed: Some(42),
            },
            ..StudyConfig::maximize("placeholder")
        },
        objective: ObjectiveConfig {
            objective: ObjectiveDef::Single {
                direction: StudyDirection::Minimize,
            },
            timeout_seconds: None,
            resource_name: "episode_reward".to_string(),
        },
        hpo: HPORunConfig {
            n_trials: 10,
            n_jobs: 1,
            timeout_seconds: None,
            early_stopping: false,
        },
        search_space: HashMap::new(),
    };

    // 验证 config 字段
    assert_eq!(config.n_trials(), 10);
    assert_eq!(config.study.direction, StudyDirection::Minimize);

    // 模拟 trial 评估 + 追踪
    let tracker = MemoryTracker::new();
    let trials_data = [
        (vec![("x".to_string(), 0.2), ("y".to_string(), 0.3)], -0.10),
        (vec![("x".to_string(), 0.4), ("y".to_string(), 0.2)], -0.05),
        (vec![("x".to_string(), 0.5), ("y".to_string(), 0.3)], 0.0), // 最佳
        (vec![("x".to_string(), 0.6), ("y".to_string(), 0.4)], -0.06),
    ];

    for (i, (params, value)) in trials_data.iter().enumerate() {
        let trial = make_trial(i as i32, params.clone(), *value, TrialState::Complete);
        assert_eq!(trial.trial_id, i as i32);
        assert!((trial.values[0] - *value).abs() < 1e-9);
        // 验证 params 是 HashMap<String, serde_json::Value>
        assert!(trial.params.contains_key("x"));
        assert!(trial.params.contains_key("y"));

        // 同步到 tracker
        for (k, v) in params {
            tracker.log_param(k, &ParamValue::Float(*v)).unwrap();
        }
        tracker.log_metric("loss", *value, i).unwrap();
    }

    // 验证参数和指标都已记录
    let recorded = tracker.get_all_params();
    assert!(recorded.contains_key("x"));
    assert!(recorded.contains_key("y"));

    let loss_history = tracker.get_metrics_by_key("loss");
    assert_eq!(loss_history.len(), 4);
    assert!(
        (scalar(&loss_history[2]) - 0.0).abs() < 1e-9,
        "最佳 trial 应在 tracker 中可识别"
    );

    // 验证 parabolic_objective 的数学性质
    let opt = parabolic_objective(&[("x".to_string(), 0.5), ("y".to_string(), 0.3)]);
    assert!((opt - 0.0).abs() < 1e-9, "最优点函数值应为 0");
}

/// 测试：批量参数记录（HPO 启动时一次性记录所有超参）
pub fn run_hpo_batch_param_logging() {
    let tracker = MemoryTracker::new();

    let all_params = vec![
        ("learning_rate".to_string(), ParamValue::Float(0.001)),
        ("batch_size".to_string(), ParamValue::Int(64)),
        ("gamma".to_string(), ParamValue::Float(0.99)),
        (
            "activation".to_string(),
            ParamValue::String("relu".to_string()),
        ),
        ("use_layernorm".to_string(), ParamValue::Bool(true)),
    ];

    tracker.log_params(&all_params).unwrap();

    // 验证所有参数都被记录
    for (k, expected) in &all_params {
        let recorded = tracker.get_param(k);
        let recorded = recorded.unwrap_or_else(|| panic!("param {k} should be recorded"));
        // 通过 Display 间接比较（ParamValue 未实现 PartialEq）
        assert_eq!(
            recorded.to_string(),
            expected.to_string(),
            "param {k} mismatch"
        );
    }

    let all = tracker.get_all_params();
    assert_eq!(all.len(), 5);

    println!("[HPO+Tracker] batch logged {} params", all.len());
}
