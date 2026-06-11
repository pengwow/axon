//! Agent 配置与错误类型

use thiserror::Error;

/// Agent 配置
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// 最大推理轮次（防止无限循环）
    pub max_iterations: usize,
    /// 温度参数
    pub temperature: f32,
    /// 上下文窗口大小
    pub max_context_tokens: usize,
    /// 是否启用反思机制
    pub enable_reflection: bool,
    /// 工具权限白名单
    pub allowed_tools: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            temperature: 0.1,
            max_context_tokens: 8192,
            enable_reflection: true,
            allowed_tools: vec![],
        }
    }
}

impl AgentConfig {
    /// 创建一个带默认值的 builder 起点
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置最大推理轮次
    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    /// 设置温度参数
    pub fn with_temperature(mut self, t: f32) -> Self {
        self.temperature = t;
        self
    }

    /// 设置上下文窗口大小
    pub fn with_max_context_tokens(mut self, n: usize) -> Self {
        self.max_context_tokens = n;
        self
    }

    /// 设置是否启用反思
    pub fn with_reflection(mut self, b: bool) -> Self {
        self.enable_reflection = b;
        self
    }

    /// 设置工具权限白名单
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }
}

/// 错误严重级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// 可恢复（应重试 / 降级）
    Recoverable,
    /// 警告（业务异常，但 Agent 可继续工作）
    Warning,
    /// 严重（必须中止）
    Critical,
}

/// Agent 错误
#[derive(Debug, Error)]
pub enum AgentError {
    /// LLM 调用失败
    #[error("LLM 调用失败: {0}")]
    LLMError(String),

    /// 工具执行失败
    #[error("工具执行失败: {0}")]
    ToolError(String),

    /// 超过最大推理轮次
    #[error("超过最大推理轮次 ({max})")]
    MaxIterationsExceeded {
        /// 配置的上限
        max: usize,
    },

    /// 权限拒绝
    #[error("权限拒绝: {0}")]
    PermissionDenied(String),

    /// 上下文窗口溢出
    #[error("上下文窗口溢出")]
    ContextOverflow,

    /// 解析响应失败
    #[error("解析响应失败: {0}")]
    ParseError(String),
}

impl AgentError {
    /// 是否可恢复（可重试或降级）
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AgentError::LLMError(_) | AgentError::ToolError(_) | AgentError::ContextOverflow
        )
    }

    /// 错误严重级别
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AgentError::LLMError(_) => ErrorSeverity::Recoverable,
            AgentError::ToolError(_) => ErrorSeverity::Recoverable,
            AgentError::ContextOverflow => ErrorSeverity::Recoverable,
            AgentError::MaxIterationsExceeded { .. } => ErrorSeverity::Warning,
            AgentError::ParseError(_) => ErrorSeverity::Warning,
            AgentError::PermissionDenied(_) => ErrorSeverity::Critical,
        }
    }
}
