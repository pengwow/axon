//! `live_trading_demo` —— axon-llm 真实 LLM 端到端 demo
//!
//! 演示:
//! 1. 从 `demo/bin/config.toml` 读 backend 配置
//! 2. 构造 `OpenAICompatBackend` 真实调 DeepSeek
//! 3. 发送一段 query,打印 response
//! 4. 跑一次"工具调用 → 解析"循环
//!
//! 运行:
//! ```bash
//! export DEEPSEEK_API_KEY=sk-...
//! cargo run -p axon-llm --example live_trading_demo --features demo
//! ```
//!
//! 退出码:
//!  0 — 成功
//!
//!  1 — 配置 / 环境错误(缺 API key、config 解析失败)
//!  2 — backend 错误(网络 / 限流 / 解析)
//!  3 — 工具执行错误

use std::path::Path;

use axon_llm::backend::{LLMBackend, LLMError, ToolDefinition};
use axon_llm::backends::{OpenAICompatBackend, OpenAICompatConfig};
use axon_llm::types::Message;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DemoConfig {
    /// LLM backend 配置
    backend: BackendSection,
    /// 演示 query
    query: String,
}

#[derive(Debug, Deserialize)]
struct BackendSection {
    base_url: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
    timeout_secs: u64,
}

fn main() {
    // 1. 读 config
    let cfg_path = Path::new("crates/axon-llm/demo/bin/config.toml");
    let cfg = match load_config(cfg_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ 加载 config 失败: {e}");
            eprintln!("   尝试从 cwd 寻找: {cfg_path:?}");
            std::process::exit(1);
        }
    };

    // 2. 拿 API key
    let api_key = match std::env::var("DEEPSEEK_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("❌ DEEPSEEK_API_KEY 未设置");
            eprintln!("   export DEEPSEEK_API_KEY=sk-... 后重试");
            std::process::exit(1);
        }
    };

    // 3. 构造 backend
    let llm_cfg = OpenAICompatConfig {
        base_url: cfg.backend.base_url.clone(),
        api_key,
        model: cfg.backend.model.clone(),
        timeout: std::time::Duration::from_secs(cfg.backend.timeout_secs),
        max_tokens: cfg.backend.max_tokens,
        temperature: cfg.backend.temperature,
        backoff: axon_llm::backends::BackoffConfig::default(),
    };
    let backend = OpenAICompatBackend::new(llm_cfg);
    println!(
        "▶ backend 初始化完成: {} (model={})",
        cfg.backend.base_url, cfg.backend.model
    );

    // 4. 启 tokio runtime 跑异步
    let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
    if let Err(e) = rt.block_on(run_demo(backend, &cfg.query)) {
        eprintln!("❌ demo 失败: {e}");
        std::process::exit(match e {
            DemoError::Backend(_) => 2,
            DemoError::Tool(_) => 3,
        });
    }
}

async fn run_demo(backend: OpenAICompatBackend, query: &str) -> Result<(), DemoError> {
    println!("\n=== 阶段 1: 简单对话 ===");
    println!("user: {query}");
    let msgs = vec![Message::user(query)];
    let resp = backend.complete(&msgs).await.map_err(DemoError::Backend)?;
    let content = resp.content.clone().unwrap_or_default();
    println!("assistant: {content}");
    println!(
        "token usage: prompt={} completion={} total={}",
        resp.token_usage.prompt_tokens,
        resp.token_usage.completion_tokens,
        resp.token_usage.total_tokens
    );

    println!("\n=== 阶段 2: 工具调用 ===");
    let tools = vec![ToolDefinition {
        name: "get_quote".into(),
        description: "Get the latest quote for a stock symbol".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "Stock ticker, e.g. AAPL"}
            },
            "required": ["symbol"]
        }),
    }];

    let tool_query = "What's the current price of AAPL? Use the get_quote tool.".to_string();
    println!("user: {tool_query}");
    let resp2 = backend
        .complete_with_tools(&[Message::user(tool_query)], &tools)
        .await
        .map_err(DemoError::Backend)?;

    if resp2.has_tool_calls() {
        let tc = &resp2.tool_calls.expect("tool_calls")[0];
        println!(
            "assistant 决定调用工具: {}({})",
            tc.function_name, tc.arguments
        );
        // 真实场景:这里会执行 broker API;demo 直接 mock 返回
        let mock_result = format!(
            r#"{{"symbol":"AAPL","price":178.42,"note":"mock result from demo (no real broker call)"}}"#
        );
        println!("tool result: {mock_result}");

        // 5. 把 tool result 喂回 LLM,获得自然语言答复
        let follow_up = vec![
            Message::user("What's the current price of AAPL? Use the get_quote tool."),
            Message::assistant(""),
            axon_llm::types::Message {
                role: axon_llm::types::Role::Assistant,
                content: String::new(),
                tool_call_id: None,
                tool_calls: Some(vec![tc.clone()]),
            },
            Message::tool_result(&tc.id, &mock_result),
        ];
        let resp3 = backend.complete(&follow_up).await.map_err(DemoError::Backend)?;
        println!("\nassistant(基于工具结果): {}", resp3.content.unwrap_or_default());
    } else {
        println!("assistant(未调用工具): {}", resp2.content.unwrap_or_default());
    }

    Ok(())
}

fn load_config(path: &Path) -> Result<DemoConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    toml::from_str(&raw).map_err(|e| format!("parse {path:?}: {e}"))
}

#[derive(Debug)]
enum DemoError {
    Backend(LLMError),
    /// 本地 tool 执行错误变体。
    ///
    /// **告警抑制决策**(按 workspace rule #4):`Tool` variant 当前已在 match arm
    /// (第 89 行 `=> 3`)和 Display impl (第 180 行)中被使用,rustc dead_code lint
    /// 不会报警,因此**不需要** `#[allow(dead_code)]`。此处保留 variant 是为未来
    /// 接入真实 broker API 时使用,无需反复改动 demo 错误类型。
    Tool(String),
}

impl std::fmt::Display for DemoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backend(e) => write!(f, "backend error: {e}"),
            Self::Tool(s) => write!(f, "tool error: {s}"),
        }
    }
}

impl std::error::Error for DemoError {}
