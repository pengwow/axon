//! LLM 后端抽象与内置实现
//!
//! - [`LLMBackend`] trait：所有后端必须实现的接口
//! - [`LLMError`]：统一错误类型
//!
//! 内置实现见 `backends` 模块（`MockBackend` 在测试代码中）。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{LLMResponse, Message};

/// LLM 错误
#[derive(Debug, Error)]
pub enum LLMError {
    /// 网络错误
    #[error("network error: {0}")]
    Network(String),

    /// 认证失败
    #[error("auth error: {0}")]
    Auth(String),

    /// 限流
    #[error("rate limited")]
    RateLimited,

    /// 响应解析失败
    #[error("parse error: {0}")]
    Parse(String),

    /// 上下文窗口溢出
    #[error("context window overflow: {needed} > {limit}")]
    ContextOverflow {
        /// 实际所需
        needed: usize,
        /// 限制
        limit: usize,
    },

    /// Mock 后端预编程响应已耗尽（仅测试用）
    #[error("mock backend responses exhausted")]
    MockExhausted,

    /// 后端通用错误
    #[error("backend error: {0}")]
    Backend(String),
}

impl LLMError {
    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Network(_) | Self::RateLimited)
    }
}

/// LLM 后端 trait
///
/// 实现了该 trait 的类型可作为 LLM 提供方接入 `ReActAgent`：
/// - 生产环境：OpenAI / Anthropic / 本地推理服务
/// - 测试环境：MockBackend
#[async_trait]
pub trait LLMBackend: Send + Sync {
    /// 发送提示并获取原始响应（无工具）
    async fn complete(&self, messages: &[Message]) -> Result<LLMResponse, LLMError>;

    /// 发送带工具定义的提示并获取结构化响应（Function Calling）
    async fn complete_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LLMResponse, LLMError>;

    /// 返回模型上下文窗口大小（token 数）
    fn context_window_size(&self) -> usize;
}

/// 工具定义（发送给 LLM 的 schema）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 参数 JSON Schema
    pub parameters: serde_json::Value,
}
