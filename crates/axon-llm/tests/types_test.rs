//! TDD 红绿循环：从最简单的类型开始
//!
//! 第一轮：Role 枚举
//! 第二轮：TokenUsage 聚合
//! 第三轮：FinishReason
//! 第四轮：ToolCall
//! 第五轮：Message
//! 第六轮：LLMResponse

use axon_llm::types::{FinishReason, LLMResponse, Message, Role, TokenUsage, ToolCall};

// ─── Role ──────────────────────────────────────────────

/// 第一个测试：枚举值与协议字符串的对应关系
#[test]
fn test_role_as_str_matches_llm_protocol() {
    assert_eq!(Role::System.as_str(), "system");
    assert_eq!(Role::User.as_str(), "user");
    assert_eq!(Role::Assistant.as_str(), "assistant");
    assert_eq!(Role::Tool.as_str(), "tool");
}

/// Role 必须支持 Hash（用于在 HashMap 中作为 key 索引会话/工具）
#[test]
fn test_role_supports_hash_and_eq() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Role::System);
    set.insert(Role::User);
    set.insert(Role::Assistant);
    set.insert(Role::Tool);

    assert_eq!(set.len(), 4);
    assert!(set.contains(&Role::System));
    assert!(!set.contains(&Role::User) || set.len() == 4);
}

/// Role 必须能 JSON 序列化往返（向后兼容协议）
#[test]
fn test_role_serde_round_trip() {
    let json = serde_json::to_string(&Role::Assistant).unwrap();
    assert_eq!(json, "\"assistant\"");

    let back: Role = serde_json::from_str(&json).unwrap();
    assert_eq!(back, Role::Assistant);
}

// ─── TokenUsage ──────────────────────────────────────────────

/// TokenUsage::default() 必须全为零（业务中常作为累加起点）
#[test]
fn test_token_usage_default_is_zero() {
    let u = TokenUsage::default();
    assert_eq!(u.prompt_tokens, 0);
    assert_eq!(u.completion_tokens, 0);
    assert_eq!(u.total_tokens, 0);
}

/// TokenUsage::new(prompt, completion) 必须自动计算 total
#[test]
fn test_token_usage_new_computes_total() {
    let u = TokenUsage::new(100, 50);
    assert_eq!(u.prompt_tokens, 100);
    assert_eq!(u.completion_tokens, 50);
    assert_eq!(u.total_tokens, 150);
}

/// TokenUsage::add 必须正确累加
#[test]
fn test_token_usage_add_accumulates() {
    let mut a = TokenUsage::new(10, 5);
    a.add(TokenUsage::new(20, 15));
    assert_eq!(a.prompt_tokens, 30);
    assert_eq!(a.completion_tokens, 20);
    assert_eq!(a.total_tokens, 50);
}

// ─── FinishReason ──────────────────────────────────────────────

/// FinishReason 必须能 JSON 往返，且为 lowercase 字符串
#[test]
fn test_finish_reason_serde_lowercase() {
    for (reason, expected) in [
        (FinishReason::Stop, "\"stop\""),
        (FinishReason::Length, "\"length\""),
        (FinishReason::ToolCalls, "\"tool_calls\""),
        (FinishReason::ContentFilter, "\"content_filter\""),
    ] {
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, expected, "reason {:?} 序列化不正确", reason);
        let back: FinishReason = serde_json::from_str(&json).unwrap();
        assert_eq!(back, reason);
    }
}

// ─── ToolCall ──────────────────────────────────────────────

/// ToolCall 字段必须正确序列化（协议兼容）
#[test]
fn test_tool_call_serde_round_trip() {
    let call = ToolCall {
        id: "call_abc".to_string(),
        function_name: "analyze_market".to_string(),
        arguments: r#"{"symbol":"BTC/USDT"}"#.to_string(),
    };
    let json = serde_json::to_string(&call).unwrap();
    let back: ToolCall = serde_json::from_str(&json).unwrap();
    assert_eq!(back, call);
}

// ─── Message ──────────────────────────────────────────────

/// 各角色构造器必须设置正确 role 与 content
#[test]
fn test_message_constructors_set_role_and_content() {
    let sys = Message::system("you are a trader");
    assert_eq!(sys.role, Role::System);
    assert_eq!(sys.content, "you are a trader");
    assert!(sys.tool_call_id.is_none());
    assert!(sys.tool_calls.is_none());

    let user = Message::user("BTC 涨了吗？");
    assert_eq!(user.role, Role::User);
    assert_eq!(user.content, "BTC 涨了吗？");

    let asst = Message::assistant("让我分析一下");
    assert_eq!(asst.role, Role::Assistant);
    assert_eq!(asst.content, "让我分析一下");

    let tool = Message::tool_result("call_1", "BTC 当前价 50000");
    assert_eq!(tool.role, Role::Tool);
    assert_eq!(tool.tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(tool.content, "BTC 当前价 50000");
}

/// Message 序列化时 Optional 字段必须被省略（与协议兼容）
#[test]
fn test_message_serialization_omits_empty_optional_fields() {
    let msg = Message::user("hello");
    let v: serde_json::Value = serde_json::to_value(&msg).unwrap();
    assert_eq!(v["role"], "user");
    assert_eq!(v["content"], "hello");
    // 协议要求不发送空的可选字段
    assert!(v.get("tool_call_id").is_none());
    assert!(v.get("tool_calls").is_none());
}

/// 携带 tool_calls 的助手消息必须能往返
#[test]
fn test_assistant_message_with_tool_calls_round_trip() {
    let call = ToolCall {
        id: "call_1".to_string(),
        function_name: "analyze_market".to_string(),
        arguments: r#"{"symbol":"BTC/USDT"}"#.to_string(),
    };
    let mut msg = Message::assistant("");
    msg.tool_calls = Some(vec![call.clone()]);

    let json = serde_json::to_string(&msg).unwrap();
    let back: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(back, msg);
    assert_eq!(back.tool_calls.as_ref().unwrap()[0], call);
}

// ─── LLMResponse ──────────────────────────────────────────────

/// LLMResponse::text 构造器：纯文本响应
#[test]
fn test_llm_response_text_constructor() {
    let resp = LLMResponse::text("answer", TokenUsage::new(5, 3));
    assert_eq!(resp.content.as_deref(), Some("answer"));
    assert!(!resp.has_tool_calls());
    assert_eq!(resp.finish_reason, FinishReason::Stop);
}

/// LLMResponse::tool_calls 构造器：工具调用响应
#[test]
fn test_llm_response_tool_calls_constructor() {
    let call = ToolCall {
        id: "c1".to_string(),
        function_name: "f".to_string(),
        arguments: "{}".to_string(),
    };
    let resp = LLMResponse::tool_calls(vec![call], TokenUsage::new(10, 2));
    assert!(resp.has_tool_calls());
    assert!(resp.content.is_none());
    assert_eq!(resp.finish_reason, FinishReason::ToolCalls);
    assert_eq!(resp.tool_calls.as_ref().unwrap().len(), 1);
}

/// has_tool_calls 必须把"空列表"判定为 false
#[test]
fn test_llm_response_empty_tool_calls_list_is_false() {
    let mut resp = LLMResponse::text("ok", TokenUsage::default());
    resp.tool_calls = Some(vec![]);
    assert!(!resp.has_tool_calls());
}
