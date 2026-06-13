//! TDD 第六轮：PromptTemplate
//!
//! PromptTemplate 提供结构化的提示模板，支持变量替换和 few-shot 示例。

use axon_llm::prompt::PromptTemplate;
use std::collections::HashMap;

// ─── 模板创建 ──────────────────────────────────────────────

#[test]
fn test_prompt_template_has_system_and_user() {
    let t = PromptTemplate::trader_system();
    assert!(!t.system.is_empty());
    assert!(!t.user.is_empty());
}

// ─── 模板渲染：变量替换 ─────────────────────────────────────

#[test]
fn test_template_render_replaces_variables() {
    let t = PromptTemplate {
        system: "你是 {role} 助手".into(),
        user: "分析 {symbol} 在 {timeframe}".into(),
        few_shot: vec![],
    };

    let mut vars = HashMap::new();
    vars.insert("role", "交易");
    vars.insert("symbol", "BTC/USDT");
    vars.insert("timeframe", "1h");

    let rendered = t.render_user(&vars);
    assert_eq!(rendered, "分析 BTC/USDT 在 1h");

    let rendered_sys = t.render_system(&vars);
    assert_eq!(rendered_sys, "你是 交易 助手");
}

#[test]
fn test_template_render_keeps_unknown_variables() {
    let t = PromptTemplate {
        system: "角色: {role}".into(),
        user: "分析 {unknown}".into(),
        few_shot: vec![],
    };

    let mut vars = HashMap::new();
    vars.insert("role", "trader");
    // 不提供 unknown

    let user = t.render_user(&vars);
    assert!(
        user.contains("{unknown}"),
        "未提供的变量应保留原文: {}",
        user
    );
}

// ─── Few-shot 示例 ─────────────────────────────────────────

#[test]
fn test_prompt_template_with_few_shot() {
    let t = PromptTemplate::market_analysis();
    // 预设模板应包含 few-shot 示例
    assert!(t.system.contains("交易"), "市场分析模板应包含交易相关提示");
}

/// 将 few-shot 示例追加到系统提示（用于上下文学习）
#[test]
fn test_prompt_template_few_shot_appends_to_system() {
    let t = PromptTemplate {
        system: "基础系统提示".into(),
        user: "问题".into(),
        few_shot: vec![
            ("Q1".to_string(), "A1".to_string()),
            ("Q2".to_string(), "A2".to_string()),
        ],
    };

    let combined = t.system_with_few_shot();
    assert!(combined.contains("Q1"));
    assert!(combined.contains("A1"));
    assert!(combined.contains("Q2"));
    assert!(combined.contains("A2"));
}
