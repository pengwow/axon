//! E2E 测试：最简场景 —— 单轮 LLM 文本对话
//!
//! - 有 `DEEPSEEK_API_KEY` + `E2E_MODE=live`  → 真实调用 DeepSeek
//! - 无 key + 默认 `E2E_MODE=replay`          → 从 fixture 读响应
//! - 无 key + `E2E_MODE=record`               → 报缺 API key
//!
//! Fixture 路径:`tests/e2e/common/fixtures/simple_chat/deepseek-chat/greeting_001.json`
//! (由 `record-fixtures.sh` 生成)

#![cfg(feature = "e2e")]

mod common;

use axon_llm::backend::LLMBackend;
use axon_llm::types::Message;

const TEST: &str = "simple_chat";
const MODEL: &str = "deepseek-chat";

#[tokio::test]
async fn greeting_001_returns_text_via_recording_layer() {
    if !common::has_key_or_fixture(TEST, MODEL) {
        eprintln!("skipping: no key + no fixture");
        return;
    }
    let backend = common::deepseek_backend().expect("DEEPSEEK_API_KEY not set");

    let layer = axon_llm::backends::RecordingLayer::from_env(TEST)
        .with_fixtures_dir(common::fixtures_dir());

    let req = axon_llm::backends::RecordedRequest {
        url: "https://api.deepseek.com/v1/chat/completions".into(),
        method: "POST".into(),
        headers: std::collections::BTreeMap::from([(
            "content-type".into(),
            "application/json".into(),
        )]),
        body: serde_json::json!({
            "model": MODEL,
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 1024,
            "temperature": 0.7,
        }),
    };

    let resp = layer.send(req, &backend).await.expect("layer send");
    assert_eq!(resp.status, 200, "expected 200, got body={}", resp.body);
    let content = resp.body["choices"][0]["message"]["content"]
        .as_str()
        .expect("choices[0].message.content should be string");
    assert!(!content.is_empty(), "content should be non-empty");

    let prompt = resp.body["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as usize;
    let completion = resp.body["usage"]["completion_tokens"]
        .as_u64()
        .unwrap_or(0) as usize;
    let usage = axon_llm::types::TokenUsage::new(prompt, completion);
    common::assert_cost_under(&usage, MODEL, 0.001);
}

#[tokio::test]
async fn greeting_via_direct_backend_returns_text() {
    if !common::has_key_or_fixture(TEST, MODEL) {
        eprintln!("skipping: no key + no fixture");
        return;
    }
    let backend = common::deepseek_backend().expect("DEEPSEEK_API_KEY not set");
    let msgs = vec![Message::user("Hello")];
    let resp = backend.complete(&msgs).await.expect("backend complete");
    let content = resp.content.expect("text response");
    assert!(!content.is_empty(), "content should be non-empty");
    common::assert_cost_under(&resp.token_usage, MODEL, 0.001);
}
