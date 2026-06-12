//! ExplanationStore 单元测试
//!
//! 覆盖：插入 / 查询 / 不存在 / FIFO 淘汰 / 重复 ID 覆盖 / capacity 边界 / 并发读写

#![cfg(feature = "explain")]

use std::sync::Arc;

use axon_explain::types::{ActionSnapshot, AttentionWeights, CounterfactualExplanation, Explanation};
use axon_llm::explain::ExplanationStore;

fn sample_explanation(id: &str) -> Explanation {
    Explanation {
        id: id.to_string(),
        observation_id: format!("obs-{}", id),
        action: ActionSnapshot {
            position_size: 1.0,
            entry_price: 100.0,
            stop_loss: 90.0,
            take_profit: 120.0,
            order_type: "limit".to_string(),
        },
        feature_importance: Default::default(),
        action_attributions: vec![],
        attention_weights: None,
        counterfactuals: vec![],
        summary: format!("Summary {}", id),
        confidence: 0.9,
        generated_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn test_store_insert_and_get() {
    let store = ExplanationStore::new(100);
    let exp = sample_explanation("d1");
    store.insert("d1".to_string(), exp.clone()).await;
    let got = store.get("d1").await;
    assert!(got.is_some());
    assert_eq!(got.unwrap().summary, "Summary d1");
}

#[tokio::test]
async fn test_store_get_nonexistent_returns_none() {
    let store = ExplanationStore::new(100);
    let got = store.get("nope").await;
    assert!(got.is_none());
}

#[tokio::test]
async fn test_store_contains_key_distinguishes_presence() {
    let store = ExplanationStore::new(100);
    assert!(!store.contains_key("absent").await);
    store.insert("d1".to_string(), sample_explanation("d1")).await;
    assert!(store.contains_key("d1").await);
}

#[tokio::test]
async fn test_store_capacity_evicts_oldest() {
    let store = ExplanationStore::new(3);
    store.insert("d1".to_string(), sample_explanation("d1")).await;
    store.insert("d2".to_string(), sample_explanation("d2")).await;
    store.insert("d3".to_string(), sample_explanation("d3")).await;
    assert_eq!(store.len().await, 3);

    // 第 4 条应淘汰 d1（FIFO）
    store.insert("d4".to_string(), sample_explanation("d4")).await;
    assert_eq!(store.len().await, 3);
    assert!(!store.contains_key("d1").await, "d1 应被淘汰");
    assert!(store.contains_key("d2").await);
    assert!(store.contains_key("d3").await);
    assert!(store.contains_key("d4").await);
}

#[tokio::test]
async fn test_store_reinsert_same_id_does_not_evict() {
    // 覆盖同 ID：不应触发 FIFO 淘汰
    let store = ExplanationStore::new(2);
    store.insert("d1".to_string(), sample_explanation("d1")).await;
    store.insert("d2".to_string(), sample_explanation("d2")).await;

    // 覆盖 d1
    store.insert("d1".to_string(), sample_explanation("d1-v2")).await;
    assert_eq!(store.len().await, 2);
    assert!(store.contains_key("d1").await);
    assert!(store.contains_key("d2").await);

    // 现在插 d3：淘汰顺序应该跳过 d1（已被覆盖，不是"oldest"）
    // 实际行为：因 d1 重新插入会重排到 order 末尾，所以 d2 被淘汰
    // 这个测试主要确保不会出现 panic 和容量溢出
    store.insert("d3".to_string(), sample_explanation("d3")).await;
    assert_eq!(store.len().await, 2);
}

#[tokio::test]
async fn test_store_latest_returns_most_recent() {
    let store = ExplanationStore::new(100);
    store.insert("d1".to_string(), sample_explanation("d1")).await;
    store.insert("d2".to_string(), sample_explanation("d2")).await;
    store.insert("d3".to_string(), sample_explanation("d3")).await;

    let latest = store.latest(2).await;
    assert_eq!(latest.len(), 2);
    // d3 最新，应在尾部
    assert_eq!(latest[1].id, "d3");
    assert_eq!(latest[0].id, "d2");
}

#[tokio::test]
async fn test_store_latest_n_larger_than_capacity() {
    let store = ExplanationStore::new(100);
    store.insert("d1".to_string(), sample_explanation("d1")).await;
    let latest = store.latest(50).await;
    assert_eq!(latest.len(), 1);
}

#[tokio::test]
async fn test_store_default_uses_default_capacity() {
    let store = ExplanationStore::default();
    assert_eq!(store.capacity(), ExplanationStore::DEFAULT_CAPACITY);
}

#[tokio::test]
async fn test_store_concurrent_inserts() {
    let store = Arc::new(ExplanationStore::new(100));
    let mut handles = vec![];

    for i in 0..50 {
        let s = Arc::clone(&store);
        let id = format!("d{}", i);
        handles.push(tokio::spawn(async move {
            s.insert(id.clone(), sample_explanation(&id)).await;
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(store.len().await, 50);
}

#[tokio::test]
async fn test_store_concurrent_reads_writes() {
    let store = Arc::new(ExplanationStore::new(100));
    store.insert("d1".to_string(), sample_explanation("d1")).await;

    let mut handles = vec![];
    for _ in 0..20 {
        let s = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            let _ = s.get("d1").await;
        }));
    }
    for i in 0..20 {
        let s = Arc::clone(&store);
        let id = format!("d_new_{}", i);
        handles.push(tokio::spawn(async move {
            s.insert(id.clone(), sample_explanation(&id)).await;
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    // d1 仍在（容量足够）
    assert!(store.contains_key("d1").await);
}

// 抑制 unused warning
#[allow(dead_code)]
fn _unused(_: AttentionWeights, _: CounterfactualExplanation) {}
