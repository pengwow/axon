//! 系统事件

use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

/// 系统事件
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemEvent {
    /// 事件序列号
    pub seq: u64,
    /// 事件时间戳
    pub timestamp: Timestamp,
    /// 系统操作
    pub action: SystemAction,
}

/// 系统操作
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SystemAction {
    /// 心跳（保活/同步）
    Heartbeat,
    /// 会话开始
    SessionStart {
        /// 会话 ID
        session_id: String,
    },
    /// 会话结束
    SessionEnd {
        /// 会话 ID
        session_id: String,
    },
    /// 错误
    Error {
        /// 错误消息
        message: String,
    },
    /// 自定义键值对
    Custom {
        /// 键
        key: String,
        /// 值
        value: String,
    },
}

impl SystemEvent {
    /// 创建系统事件
    pub fn new(seq: u64, timestamp: Timestamp, action: SystemAction) -> Self {
        Self {
            seq,
            timestamp,
            action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_event_heartbeat() {
        let ts = Timestamp::from_nanos(1_000);
        let event = SystemEvent::new(0, ts, SystemAction::Heartbeat);
        assert_eq!(event.action, SystemAction::Heartbeat);
    }

    #[test]
    fn test_system_event_session() {
        let ts = Timestamp::from_nanos(1_000);
        let event = SystemEvent::new(
            0,
            ts,
            SystemAction::SessionStart {
                session_id: "sess-1".to_string(),
            },
        );
        match event.action {
            SystemAction::SessionStart { session_id } => {
                assert_eq!(session_id, "sess-1");
            }
            _ => panic!("expected SessionStart"),
        }
    }

    #[test]
    fn test_system_event_error() {
        let event = SystemEvent::new(
            0,
            Timestamp::from_nanos(0),
            SystemAction::Error {
                message: "boom".to_string(),
            },
        );
        match event.action {
            SystemAction::Error { message } => assert_eq!(message, "boom"),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn test_system_event_custom() {
        let event = SystemEvent::new(
            0,
            Timestamp::from_nanos(0),
            SystemAction::Custom {
                key: "region".to_string(),
                value: "us-east-1".to_string(),
            },
        );
        match event.action {
            SystemAction::Custom { key, value } => {
                assert_eq!(key, "region");
                assert_eq!(value, "us-east-1");
            }
            _ => panic!("expected Custom"),
        }
    }
}
