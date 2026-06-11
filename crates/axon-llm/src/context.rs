//! 上下文窗口管理 + 对话记忆
//!
//! - [`ContextManager`]：跟踪当前会话的 token 用量，超出限制时自动压缩最旧消息
//! - [`ConversationMemory`]：保留完整对话历史（用于审计、回溯）

use crate::types::{Message, Role};

// ─── 上下文窗口管理器 ─────────────────────────────────────────

/// 上下文窗口管理器
pub struct ContextManager {
    /// 最大 token 数
    max_tokens: usize,
    /// 当前消息列表
    messages: Vec<Message>,
    /// 当前 token 估算
    current_tokens: usize,
    /// 估算每 token 的字符数（中英文混合约 2-3）
    chars_per_token: f64,
}

impl ContextManager {
    /// 创建新的上下文管理器
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            messages: Vec::new(),
            current_tokens: 0,
            // 中英文混合经验值
            chars_per_token: 2.5,
        }
    }

    /// 添加消息，自动管理上下文窗口
    pub fn add_message(&mut self, message: Message) {
        let estimated_tokens = self.estimate_tokens(&message.content);

        // 循环压缩直至装下为止（但至少保留 system + 当前消息）
        while self.current_tokens + estimated_tokens > self.max_tokens && self.messages.len() > 1 {
            self.compress_oldest_message();
        }

        self.current_tokens += estimated_tokens;
        self.messages.push(message);
    }

    /// 估算 token 数
    fn estimate_tokens(&self, text: &str) -> usize {
        (text.chars().count() as f64 / self.chars_per_token).ceil() as usize
    }

    /// 压缩最旧可压缩的消息（保留 system 消息在首位）
    fn compress_oldest_message(&mut self) {
        // 找到第一个非 system 消息进行压缩
        let target = self
            .messages
            .iter()
            .position(|m| m.role != Role::System);

        let idx = match target {
            Some(i) if i > 0 => i,
            // 只有 system 消息或只有一条 - 不压缩
            _ => return,
        };

        let original = &self.messages[idx];
        let original_tokens = self.estimate_tokens(&original.content);

        let summary = format!("[历史摘要] {}", self.extract_topic(&original.content));
        let summary_tokens = self.estimate_tokens(&summary);

        // 如果摘要不比原消息短，则放弃压缩（避免无效压缩循环）
        if summary_tokens >= original_tokens {
            return;
        }

        self.messages[idx] = Message {
            role: Role::User,
            content: summary,
            tool_call_id: None,
            tool_calls: None,
        };

        self.current_tokens = self.current_tokens.saturating_sub(original_tokens) + summary_tokens;
    }

    /// 从消息中提取主题（简单实现：截断前 N 字符）
    fn extract_topic(&self, content: &str) -> String {
        const TOPIC_LEN: usize = 30;
        let chars: String = content.chars().take(TOPIC_LEN).collect();
        if content.chars().count() >= TOPIC_LEN {
            format!("{}…", chars)
        } else {
            chars
        }
    }

    /// 获取消息列表
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// 获取当前 token 使用量
    pub fn token_usage(&self) -> usize {
        self.current_tokens
    }

    /// 获取 token 上限
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }
}

// ─── 对话记忆 ──────────────────────────────────────────────────

/// 对话记忆
pub struct ConversationMemory {
    /// 完整对话历史
    history: Vec<Message>,
    /// 最大历史长度
    max_history: usize,
}

impl ConversationMemory {
    /// 创建默认（保留 1000 条）的对话记忆
    pub fn new() -> Self {
        Self::with_max_history(1000)
    }

    /// 创建指定最大历史长度的对话记忆
    pub fn with_max_history(max: usize) -> Self {
        Self {
            history: Vec::new(),
            max_history: max,
        }
    }

    /// 添加一条消息到历史
    pub fn add(&mut self, message: Message) {
        self.history.push(message);
        // 超出限制时移除最旧
        if self.history.len() > self.max_history {
            let excess = self.history.len() - self.max_history;
            self.history.drain(0..excess);
        }
    }

    /// 获取完整历史
    pub fn history(&self) -> &[Message] {
        &self.history
    }

    /// 获取最近 n 条消息
    pub fn last_n(&self, n: usize) -> &[Message] {
        let start = self.history.len().saturating_sub(n);
        &self.history[start..]
    }

    /// 历史长度
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

impl Default for ConversationMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compress_topic_truncates_long() {
        let cm = ContextManager::new(100);
        // 确保超过 30 字符阈值（中文按字符计）
        let topic = cm.extract_topic("这是一段非常非常长的消息内容，超过了30个字符的显示限制范围");
        assert!(topic.chars().count() <= 32); // 30 + "…"
        assert!(topic.ends_with('…'));
    }

    #[test]
    fn compress_topic_keeps_short() {
        let cm = ContextManager::new(100);
        let topic = cm.extract_topic("短消息");
        assert_eq!(topic, "短消息");
    }
}
