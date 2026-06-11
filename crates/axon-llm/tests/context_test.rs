//! TDD 第五轮：ContextManager + ConversationMemory
//!
//! ContextManager 控制 LLM 上下文窗口大小，超出限制时自动压缩最旧消息。
//! ConversationMemory 保存完整对话历史（用于审计），与上下文管理器互补。

use axon_llm::context::{ContextManager, ConversationMemory};
use axon_llm::types::{Message, Role};

// ─── ContextManager 基础 ──────────────────────────────────────

#[test]
fn test_context_manager_starts_empty() {
    let cm = ContextManager::new(1000);
    assert_eq!(cm.messages().len(), 0);
    assert_eq!(cm.token_usage(), 0);
}

#[test]
fn test_context_manager_add_message_increments_usage() {
    let mut cm = ContextManager::new(1000);
    cm.add_message(Message::user("hello world"));
    assert_eq!(cm.messages().len(), 1);
    assert!(cm.token_usage() > 0, "添加消息后应有 token 用量");
}

#[test]
fn test_context_manager_preserves_message_order() {
    let mut cm = ContextManager::new(1000);
    cm.add_message(Message::system("sys"));
    cm.add_message(Message::user("u1"));
    cm.add_message(Message::assistant("a1"));

    let msgs = cm.messages();
    assert_eq!(msgs[0].role, Role::System);
    assert_eq!(msgs[1].role, Role::User);
    assert_eq!(msgs[2].role, Role::Assistant);
    assert_eq!(msgs[1].content, "u1");
}

// ─── ContextManager 压缩：超出窗口 ─────────────────────────────

#[test]
fn test_context_manager_compresses_when_overflow() {
    // 极小窗口：每条消息约 10 字符 = 3 tokens，4 字符/token
    let mut cm = ContextManager::new(20);

    // 添加 4 条长消息，必然溢出
    cm.add_message(Message::user("1234567890"));
    let usage_before = cm.token_usage();
    cm.add_message(Message::user("1234567890"));
    cm.add_message(Message::user("1234567890"));
    cm.add_message(Message::user("1234567890"));

    // 压缩后应仍 ≤ 窗口，且至少保留了最近的几条
    assert!(
        cm.token_usage() <= 20,
        "压缩后 token 用量 {} 仍超过窗口 20",
        cm.token_usage()
    );
    assert!(cm.messages().len() >= 2, "至少保留两条消息");
    assert!(usage_before > 0);
}

#[test]
fn test_context_manager_keeps_system_prompt() {
    // 系统消息本身 ~3 tokens；设置足够大的窗口容纳 2 条短消息
    let mut cm = ContextManager::new(25);
    cm.add_message(Message::system("你是交易助手")); // ~3 tokens
    cm.add_message(Message::user("u1")); // ~1 token
    cm.add_message(Message::user("u2")); // ~1 token
    cm.add_message(Message::user("u3")); // ~1 token
    cm.add_message(Message::user("u4")); // ~1 token → 超过 25，触发压缩

    let msgs = cm.messages();
    // system 必须在最前
    assert_eq!(msgs[0].role, Role::System, "system 消息必须保留在首位");
    assert_eq!(msgs[0].content, "你是交易助手");
}

// ─── ConversationMemory ─────────────────────────────────────

#[test]
fn test_conversation_memory_starts_empty() {
    let mem = ConversationMemory::new();
    assert_eq!(mem.len(), 0);
}

#[test]
fn test_conversation_memory_add_records_in_order() {
    let mut mem = ConversationMemory::new();
    mem.add(Message::user("u1"));
    mem.add(Message::assistant("a1"));
    mem.add(Message::user("u2"));
    assert_eq!(mem.len(), 3);
    assert_eq!(mem.history()[0].content, "u1");
    assert_eq!(mem.history()[2].content, "u2");
}

#[test]
fn test_conversation_memory_get_last_n() {
    let mut mem = ConversationMemory::new();
    for i in 0..5 {
        mem.add(Message::user(format!("m{}", i)));
    }
    let last2 = mem.last_n(2);
    assert_eq!(last2.len(), 2);
    assert_eq!(last2[0].content, "m3");
    assert_eq!(last2[1].content, "m4");
}

#[test]
fn test_conversation_memory_respects_max_history() {
    let mut mem = ConversationMemory::with_max_history(3);
    for i in 0..10 {
        mem.add(Message::user(format!("m{}", i)));
    }
    assert_eq!(mem.len(), 3, "超出 max_history 时应截断");
    // 保留最新的 3 条
    assert_eq!(mem.history()[0].content, "m7");
    assert_eq!(mem.history()[2].content, "m9");
}
