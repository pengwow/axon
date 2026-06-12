//! DecisionRecorder 单元测试
//!
//! 覆盖：立即返回（不阻塞）/ 最终写入 / 重复 ID 覆盖 / 错误吞掉

#![cfg(feature = "explain")]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axon_explain::error::ExplainabilityError;
use axon_explain::traits::Explainer;
use axon_explain::types::{ActionSnapshot, AttentionWeights, CounterfactualExplanation, Explanation};
use axon_llm::explain::{DecisionRecord, DecisionRecorder, ExplainerBridge, ExplanationStore, ExplainMode};

struct SlowExplainer {
    delay: Duration,
    fail: bool,
}

#[async_trait]
impl Explainer for SlowExplainer {
    fn explain(
        &self,
        _o: &HashMap<String, f64>,
        a: &ActionSnapshot,
    ) -> Result<Explanation, ExplainabilityError> {
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
        if self.fail {
            return Err(ExplainabilityError::FeatureMismatch { expected: 1, actual: 2 });
        }
        Ok(Explanation {
            id: "slow".to_string(),
            observation_id: "obs".to_string(),
            action: a.clone(),
            feature_importance: Default::default(),
            action_attributions: vec![],
            attention_weights: None,
            counterfactuals: vec![],
            summary: "slow explanation".to_string(),
            confidence: 0.5,
            generated_at: chrono::Utc::now(),
        })
    }
    fn explain_action_dimension(
        &self,
        _o: &HashMap<String, f64>,
        _a: &ActionSnapshot,
        _d: &str,
    ) -> Result<axon_explain::types::ActionAttribution, ExplainabilityError> {
        unimplemented!()
    }
    fn get_attention_weights(
        &self,
        _o: &HashMap<String, f64>,
    ) -> Option<Vec<AttentionWeights>> {
        None
    }
    fn generate_counterfactuals(
        &self,
        _o: &HashMap<String, f64>,
        _a: &ActionSnapshot,
        _m: usize,
    ) -> Vec<CounterfactualExplanation> {
        vec![]
    }
}

fn sample_record(id: &str) -> DecisionRecord {
    DecisionRecord::new(
        id,
        ExplainMode::ActionOnly,
        "test",
        ActionSnapshot {
            position_size: 1.0,
            entry_price: 100.0,
            stop_loss: 90.0,
            take_profit: 120.0,
            order_type: "limit".to_string(),
        },
    )
}

fn make_recorder(delay: Duration, fail: bool) -> (Arc<ExplanationStore>, DecisionRecorder) {
    let store = Arc::new(ExplanationStore::new(100));
    let explainer: Arc<dyn Explainer> = Arc::new(SlowExplainer { delay, fail });
    let bridge = Arc::new(ExplainerBridge::new(explainer, Arc::clone(&store)));
    let recorder = DecisionRecorder::new(bridge);
    (store, recorder)
}

#[tokio::test]
async fn test_recorder_does_not_block_caller() {
    let (_store, recorder) = make_recorder(Duration::from_millis(300), false);

    let start = std::time::Instant::now();
    recorder.record(sample_record("r1"));
    let elapsed = start.elapsed();

    // record() 必须 < 50ms（spawn 异步，不等待 explain）
    assert!(
        elapsed.as_millis() < 50,
        "record 阻塞 {}ms，违反 fire-and-forget 语义",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn test_recorder_eventually_writes_to_store() {
    let (store, recorder) = make_recorder(Duration::from_millis(100), false);

    recorder.record(sample_record("r2"));
    // 给 spawn_blocking 任务时间完成
    tokio::time::sleep(Duration::from_millis(400)).await;

    let exp = store.get("r2").await;
    assert!(exp.is_some(), "400ms 后 store 应有 r2");
    assert_eq!(exp.unwrap().summary, "slow explanation");
}

#[tokio::test]
async fn test_recorder_multiple_records_all_land() {
    let (store, recorder) = make_recorder(Duration::from_millis(50), false);

    for i in 0..10 {
        recorder.record(sample_record(&format!("r{}", i)));
    }
    // 等所有 spawn 任务完成
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert_eq!(store.len().await, 10);
}

#[tokio::test]
async fn test_recorder_swallows_explainer_errors() {
    // fail=true 时 Explainer 返回 Err，Recorder 不应 panic 或影响调用方
    let (store, recorder) = make_recorder(Duration::ZERO, true);

    recorder.record(sample_record("err1"));
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 失败时 store 保持空（不写入）
    assert!(!store.contains_key("err1").await);
    assert!(store.is_empty().await);
}

#[tokio::test]
async fn test_recorder_bridge_clone_shares_state() {
    // 验证 bridge_clone 暴露让外部能拿到同一 bridge 构造 Compute Tool
    let (store, recorder) = make_recorder(Duration::from_millis(50), false);
    let bridge_clone = recorder.bridge_clone();

    // 用 clone 出来的 bridge 显式调用
    bridge_clone.explain_async(sample_record("via-clone")).await.unwrap();
    let exp = store.get("via-clone").await;
    assert!(exp.is_some());
}

#[tokio::test]
async fn test_recorder_duplicate_id_keeps_latest() {
    let (store, recorder) = make_recorder(Duration::from_millis(50), false);

    recorder.record(sample_record("dup"));
    recorder.record(sample_record("dup"));
    tokio::time::sleep(Duration::from_millis(300)).await;

    // store 覆盖语义：仅 1 条
    assert_eq!(store.len().await, 1);
}
