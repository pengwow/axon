//! 不可变审计日志（区块链式哈希链）
//!
//! 使用 SHA-256 哈希链确保日志不可篡改。
//! 每个事件包含前一个事件的哈希，形成链式结构。

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::error::ComplianceResult;
use crate::types::{AuditEvent, AuditEventType};

/// 不可变审计日志
#[derive(Debug, Clone)]
pub struct AuditLog {
    /// 日志条目
    entries: Vec<AuditEvent>,
    /// 最后一个事件的哈希
    last_hash: String,
}

impl AuditLog {
    /// 创建新的审计日志
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_hash: String::new(),
        }
    }

    /// 记录事件（自动计算哈希链）
    pub fn log_event(&mut self, mut event: AuditEvent) -> ComplianceResult<()> {
        // 设置前向哈希
        event.previous_hash = self.last_hash.clone();

        // 计算事件哈希
        event.event_hash = Self::compute_event_hash(&event);

        // 更新最后哈希
        self.last_hash = event.event_hash.clone();

        // 添加到日志
        self.entries.push(event);

        Ok(())
    }

    /// 验证日志完整性
    pub fn verify_integrity(&self) -> bool {
        if self.entries.is_empty() {
            return true;
        }

        let mut prev_hash = String::new();
        for entry in &self.entries {
            // 验证前向哈希
            if entry.previous_hash != prev_hash {
                return false;
            }

            // 验证事件哈希
            let computed_hash = Self::compute_event_hash(entry);
            if computed_hash != entry.event_hash {
                return false;
            }

            prev_hash = entry.event_hash.clone();
        }

        true
    }

    /// 获取事件数量
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 获取最后一个事件的哈希
    pub fn last_hash(&self) -> &str {
        &self.last_hash
    }

    /// 按时间范围查询事件
    pub fn query_events(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<&AuditEvent> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }

    /// 按事件类型查询
    pub fn query_by_type(&self, event_type: &AuditEventType) -> Vec<&AuditEvent> {
        self.entries
            .iter()
            .filter(|e| e.event_type == *event_type)
            .collect()
    }

    /// 获取所有事件
    pub fn entries(&self) -> &[AuditEvent] {
        &self.entries
    }

    /// 计算事件哈希
    fn compute_event_hash(event: &AuditEvent) -> String {
        let mut hasher = Sha256::new();

        // 序列化事件（排除 event_hash 字段）
        let mut event_for_hash = event.clone();
        event_for_hash.event_hash = String::new();

        hasher.update(serde_json::to_vec(&event_for_hash).unwrap_or_default());
        hasher.update(event.previous_hash.as_bytes());

        format!("{:x}", hasher.finalize())
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_event(action: &str) -> AuditEvent {
        AuditEvent {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::TradeExecuted,
            actor: "test_strategy".into(),
            action: action.into(),
            resource_type: "trade".into(),
            resource_id: Uuid::new_v4().to_string(),
            details: serde_json::json!({}),
            previous_hash: String::new(),
            event_hash: String::new(),
            ip_address: None,
            session_id: None,
        }
    }

    #[test]
    fn test_new_log_is_empty() {
        let log = AuditLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert_eq!(log.last_hash(), "");
    }

    #[test]
    fn test_log_event_adds_entry() {
        let mut log = AuditLog::new();
        let event = create_test_event("test_action");

        log.log_event(event).unwrap();

        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());
        assert!(!log.last_hash().is_empty());
    }

    #[test]
    fn test_chain_integrity() {
        let mut log = AuditLog::new();

        // 添加多个事件
        for i in 0..5 {
            let event = create_test_event(&format!("action_{}", i));
            log.log_event(event).unwrap();
        }

        // 验证完整性
        assert!(log.verify_integrity());
    }

    #[test]
    fn test_tamper_detection() {
        let mut log = AuditLog::new();

        // 添加事件
        let event1 = create_test_event("action_1");
        log.log_event(event1).unwrap();

        let event2 = create_test_event("action_2");
        log.log_event(event2).unwrap();

        // 篡改第一个事件的 action
        let mut tampered_entries = log.entries().to_vec();
        tampered_entries[0].action = "tampered".into();

        // 创建篡改后的日志
        let tampered_log = AuditLog {
            entries: tampered_entries,
            last_hash: log.last_hash().to_string(),
        };

        // 应该检测到篡改
        assert!(!tampered_log.verify_integrity());
    }

    #[test]
    fn test_query_by_time_range() {
        let mut log = AuditLog::new();

        let now = Utc::now();
        let event1 = AuditEvent {
            timestamp: now - chrono::Duration::hours(2),
            ..create_test_event("old_event")
        };
        let event2 = AuditEvent {
            timestamp: now,
            ..create_test_event("new_event")
        };

        log.log_event(event1).unwrap();
        log.log_event(event2).unwrap();

        // 查询最近 1 小时的事件
        let recent = log.query_events(
            now - chrono::Duration::hours(1),
            now + chrono::Duration::hours(1),
        );
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].action, "new_event");
    }

    #[test]
    fn test_query_by_type() {
        let mut log = AuditLog::new();

        let event1 = AuditEvent {
            event_type: AuditEventType::TradeExecuted,
            ..create_test_event("trade")
        };
        let event2 = AuditEvent {
            event_type: AuditEventType::OrderPlaced,
            ..create_test_event("order")
        };

        log.log_event(event1).unwrap();
        log.log_event(event2).unwrap();

        let trades = log.query_by_type(&AuditEventType::TradeExecuted);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].action, "trade");
    }
}
