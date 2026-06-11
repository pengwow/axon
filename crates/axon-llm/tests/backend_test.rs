//! TDD 第二轮：LLMBackend trait + MockBackend
//!
//! LLMBackend 是抽象层 - 必须支持纯文本和工具调用两种调用模式。
//! MockBackend 是测试基础设施 - 允许预编程响应序列。

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use axon_llm::{LLMBackend, LLMError, ToolDefinition};
use axon_llm::types::{FinishReason, LLMResponse, Message, TokenUsage, ToolCall};

// ─── 异步测试：纯文本调用 ─────────────────────────────────────

#[tokio::test]
async fn test_backend_complete_returns_text_response() {
    let backend = MockBackend::with_responses(vec![LLMResponse::text(
        "hello",
        TokenUsage::new(2, 1),
    )]);

    let resp = backend
        .complete(&[Message::user("hi")])
        .await
        .expect("调用应成功");

    assert_eq!(resp.content.as_deref(), Some("hello"));
    assert_eq!(resp.finish_reason, FinishReason::Stop);
    assert_eq!(resp.token_usage.total_tokens, 3);
}

/// complete_with_tools 在没有工具调用时应返回文本
#[tokio::test]
async fn test_backend_complete_with_tools_returns_text() {
    let backend = MockBackend::with_responses(vec![LLMResponse::text(
        "no tool needed",
        TokenUsage::default(),
    )]);
    let tools = vec![ToolDefinition {
        name: "noop".to_string(),
        description: "does nothing".to_string(),
        parameters: serde_json::json!({}),
    }];

    let resp = backend
        .complete_with_tools(&[Message::user("hi")], &tools)
        .await
        .unwrap();

    assert_eq!(resp.content.as_deref(), Some("no tool needed"));
}

/// 预编程的响应必须按顺序消费（模拟多轮对话）
#[tokio::test]
async fn test_mock_backend_consumes_responses_in_order() {
    let backend = MockBackend::with_responses(vec![
        LLMResponse::tool_calls(
            vec![ToolCall {
                id: "c1".to_string(),
                function_name: "f".to_string(),
                arguments: "{}".to_string(),
            }],
            TokenUsage::new(5, 2),
        ),
        LLMResponse::text("done", TokenUsage::new(8, 1)),
    ]);

    let r1 = backend.complete(&[]).await.unwrap();
    assert!(r1.has_tool_calls());

    let r2 = backend.complete(&[]).await.unwrap();
    assert_eq!(r2.content.as_deref(), Some("done"));
}

/// 当预编程响应耗尽时，必须返回明确错误
#[tokio::test]
async fn test_mock_backend_returns_error_when_exhausted() {
    let backend = MockBackend::with_responses(vec![]);
    let result = backend.complete(&[]).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LLMError::MockExhausted => {}
        other => panic!("应为 MockExhausted，实际为 {:?}", other),
    }
}

/// 调用计数必须正确累加
#[tokio::test]
async fn test_mock_backend_records_call_count() {
    let backend = MockBackend::with_responses(vec![
        LLMResponse::text("a", TokenUsage::default()),
        LLMResponse::text("b", TokenUsage::default()),
    ]);
    backend.complete(&[]).await.unwrap();
    backend.complete(&[]).await.unwrap();
    assert_eq!(backend.call_count().await, 2);
}

/// context_window_size 必须返回 backend 报告的值
#[tokio::test]
async fn test_backend_reports_context_window_size() {
    let backend = MockBackend::with_context_window(8192);
    assert_eq!(backend.context_window_size(), 8192);
}

/// 工具定义必须随消息一起传递给 backend（用于审计 / mock 校验）
#[tokio::test]
async fn test_mock_backend_records_last_tool_definitions() {
    let backend = MockBackend::with_responses(vec![LLMResponse::text(
        "ok",
        TokenUsage::default(),
    )]);
    let tools = vec![ToolDefinition {
        name: "ping".to_string(),
        description: "ping".to_string(),
        parameters: serde_json::json!({"type": "object"}),
    }];
    backend.complete_with_tools(&[], &tools).await.unwrap();
    let recorded = backend.last_tools().await;
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].name, "ping");
}

// ─── Mock 实现（与测试放在同一文件以验证 TDD 完整性） ───

/// 预编程响应的 MockBackend
struct MockBackend {
    /// 预编程响应队列
    responses: Arc<Mutex<VecDeque<LLMResponse>>>,
    /// 调用次数
    call_count: Arc<Mutex<usize>>,
    /// 上下文窗口大小
    context_window: usize,
    /// 最近一次调用的工具定义
    last_tools: Arc<Mutex<Vec<ToolDefinition>>>,
}

impl MockBackend {
    fn with_responses(responses: Vec<LLMResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            call_count: Arc::new(Mutex::new(0)),
            context_window: 4096,
            last_tools: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_context_window(size: usize) -> Self {
        let mut s = Self::with_responses(vec![LLMResponse::text("ok", TokenUsage::default())]);
        s.context_window = size;
        s
    }

    async fn call_count(&self) -> usize {
        *self.call_count.lock().await
    }

    async fn last_tools(&self) -> Vec<ToolDefinition> {
        self.last_tools.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl LLMBackend for MockBackend {
    async fn complete(&self, _messages: &[Message]) -> Result<LLMResponse, LLMError> {
        let mut count = self.call_count.lock().await;
        *count += 1;
        drop(count);

        let mut queue = self.responses.lock().await;
        queue.pop_front().ok_or(LLMError::MockExhausted)
    }

    async fn complete_with_tools(
        &self,
        _messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LLMResponse, LLMError> {
        {
            let mut last = self.last_tools.lock().await;
            *last = tools.to_vec();
        }
        self.complete(_messages).await
    }

    fn context_window_size(&self) -> usize {
        self.context_window
    }
}
