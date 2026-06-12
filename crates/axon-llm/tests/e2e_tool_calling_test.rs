//! E2E 测试：工具调用场景 —— LLM 决定调用 `submit_order`
//!
//! Fixture 路径:`tests/e2e/common/fixtures/tool_calling/deepseek-chat/submit_order_001.json`

#![cfg(feature = "e2e")]

mod common;

use axon_llm::backend::LLMBackend;
use axon_llm::types::Message;

const TEST: &str = "tool_calling";
const MODEL: &str = "deepseek-chat";

#[tokio::test]
async fn submit_order_tool_call_extracted() {
    if !common::has_key_or_fixture(TEST, MODEL) {
        eprintln!("skipping: no key + no fixture");
        return;
    }
    let backend = common::deepseek_backend().expect("DEEPSEEK_API_KEY not set");

    let tools = vec![axon_llm::backend::ToolDefinition {
        name: "submit_order".into(),
        description: "Submit a trade order to the broker".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string"},
                "side": {"type": "string", "enum": ["buy", "sell"]},
                "quantity": {"type": "integer", "minimum": 1},
                "order_type": {"type": "string", "enum": ["market", "limit"]}
            },
            "required": ["symbol", "side", "quantity", "order_type"]
        }),
    }];

    let msgs = vec![Message::user(
        "Submit a market order to buy 100 shares of AAPL at market price",
    )];
    let resp = backend
        .complete_with_tools(&msgs, &tools)
        .await
        .expect("complete_with_tools");

    assert!(
        resp.has_tool_calls(),
        "expected tool_calls, got content={:?}",
        resp.content
    );
    let tc = &resp.tool_calls.expect("tool_calls")[0];
    assert_eq!(tc.function_name, "submit_order");

    // 解析 arguments
    let args: serde_json::Value =
        serde_json::from_str(&tc.arguments).expect("tool args should be valid JSON");
    assert_eq!(args["symbol"], "AAPL");
    assert_eq!(args["side"], "buy");
    assert_eq!(args["quantity"], 100);

    // 成本预算
    common::assert_cost_under(&resp.token_usage, MODEL, 0.005);
}
