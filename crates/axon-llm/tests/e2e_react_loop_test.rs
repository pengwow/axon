//! E2E 测试：多轮 ReAct —— LLM 调用工具后,基于 Observation 给出最终答案
//!
//! Fixture 路径:`tests/e2e/common/fixtures/react_loop/deepseek-chat/step1.json`
//! 模拟场景:
//!   user: "AAPL 多少钱?"
//!   assistant(tool_call): get_price("AAPL")
//!   tool: {"price":178.42}
//!   assistant(text): "AAPL 当前 $178.42, 是否要买入?"

#![cfg(feature = "e2e")]

mod common;

use axon_llm::backend::LLMBackend;
use axon_llm::types::{Message, Role, ToolCall};

const TEST: &str = "react_loop";
const MODEL: &str = "deepseek-chat";

#[tokio::test]
async fn react_loop_with_tool_result_yields_text() {
    if !common::has_key_or_fixture(TEST, MODEL) {
        eprintln!("skipping: no key + no fixture");
        return;
    }
    let backend = common::deepseek_backend().expect("DEEPSEEK_API_KEY not set");

    // 多轮 messages:system + user + assistant(tool_call) + tool(result)
    let messages = vec![
        Message::system("You are a trading assistant. Use the tools to gather data and place trades."),
        Message::user("What is the current price of AAPL?"),
        Message {
            role: Role::Assistant,
            content: String::new(),
            tool_call_id: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_price_001".into(),
                function_name: "get_price".into(),
                arguments: r#"{"symbol":"AAPL"}"#.into(),
            }]),
        },
        Message::tool_result("call_price_001", r#"{"symbol":"AAPL","price":178.42}"#),
    ];

    let resp = backend.complete(&messages).await.expect("backend complete");
    let content = resp.content.expect("text response after tool result");
    assert!(!content.is_empty(), "content should be non-empty");
    // 简易断言:真实 LLM 必然提到价格
    assert!(
        content.contains("178") || content.to_lowercase().contains("price"),
        "expected mention of price/178, got: {content}"
    );

    common::assert_cost_under(&resp.token_usage, MODEL, 0.01);
}
