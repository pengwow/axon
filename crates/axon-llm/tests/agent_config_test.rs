//! TDD 第四轮：AgentConfig + AgentError
//!
//! AgentConfig 控制 ReAct 主循环的行为参数。
//! AgentError 是 Agent 内部错误的统一类型，含可恢复性/严重性分类。

use axon_llm::agent::{AgentConfig, AgentError};

// ─── AgentConfig 默认值与字段 ────────────────────────────────

#[test]
fn test_agent_config_default_is_safe() {
    let c = AgentConfig::default();
    // 默认不应无限循环
    assert!(c.max_iterations > 0 && c.max_iterations <= 100);
    // 默认温度应偏低（更确定）
    assert!(c.temperature >= 0.0 && c.temperature <= 1.0);
    // 上下文窗口必须为正
    assert!(c.max_context_tokens > 0);
}

#[test]
fn test_agent_config_builder_chains_all_fields() {
    let c = AgentConfig::new()
        .with_max_iterations(5)
        .with_temperature(0.7)
        .with_max_context_tokens(16384)
        .with_reflection(false)
        .with_allowed_tools(vec!["analyze_market".into(), "check_portfolio".into()]);

    assert_eq!(c.max_iterations, 5);
    assert!((c.temperature - 0.7).abs() < 1e-6);
    assert_eq!(c.max_context_tokens, 16384);
    assert!(!c.enable_reflection);
    assert_eq!(c.allowed_tools.len(), 2);
    assert!(c.allowed_tools.contains(&"analyze_market".to_string()));
}

// ─── AgentError 错误信息 ──────────────────────────────────────

#[test]
fn test_agent_error_llm_error_displays_cause() {
    let e = AgentError::LLMError("connection refused".into());
    let msg = e.to_string();
    assert!(msg.contains("connection refused"));
}

#[test]
fn test_agent_error_max_iterations_includes_max() {
    let e = AgentError::MaxIterationsExceeded { max: 10 };
    let msg = e.to_string();
    assert!(msg.contains("10"));
}

#[test]
fn test_agent_error_permission_denied_displays_reason() {
    let e = AgentError::PermissionDenied("submit_order 不在白名单".into());
    let msg = e.to_string();
    assert!(msg.contains("submit_order"));
}

// ─── AgentError 分类 ─────────────────────────────────────────

#[test]
fn test_agent_error_is_recoverable() {
    assert!(AgentError::LLMError("net".into()).is_recoverable());
    assert!(AgentError::ToolError("timeout".into()).is_recoverable());
    assert!(AgentError::ContextOverflow.is_recoverable());

    // 不可恢复
    assert!(!AgentError::MaxIterationsExceeded { max: 5 }.is_recoverable());
    assert!(!AgentError::PermissionDenied("x".into()).is_recoverable());
    assert!(!AgentError::ParseError("bad".into()).is_recoverable());
}

#[test]
fn test_agent_error_severity_classification() {
    use axon_llm::agent::ErrorSeverity;
    assert_eq!(
        AgentError::LLMError("x".into()).severity(),
        ErrorSeverity::Recoverable
    );
    assert_eq!(
        AgentError::MaxIterationsExceeded { max: 3 }.severity(),
        ErrorSeverity::Warning
    );
    assert_eq!(
        AgentError::PermissionDenied("x".into()).severity(),
        ErrorSeverity::Critical
    );
    assert_eq!(
        AgentError::ParseError("x".into()).severity(),
        ErrorSeverity::Warning
    );
}
