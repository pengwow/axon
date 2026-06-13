//! 文件系统存储
//!
//! 将审计日志持久化到文件系统，使用 JSONL 格式按日期分片。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use crate::error::{ComplianceError, ComplianceResult};
use crate::types::AuditEvent;

/// 文件系统存储
#[derive(Debug, Clone)]
pub struct FileStorage {
    /// 存储根目录
    base_path: PathBuf,
}

impl FileStorage {
    /// 创建新的文件系统存储
    pub fn new(base_path: impl AsRef<Path>) -> ComplianceResult<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // 创建目录结构
        fs::create_dir_all(base_path.join("audit_logs"))
            .map_err(|e| ComplianceError::StorageError(format!("create audit_logs dir: {}", e)))?;

        Ok(Self { base_path })
    }

    /// 保存审计事件
    pub fn save_event(&self, event: &AuditEvent) -> ComplianceResult<()> {
        let date = event.timestamp.date_naive();
        let path = self.audit_log_path(date);

        // 序列化事件
        let json = serde_json::to_string(event)
            .map_err(|e| ComplianceError::SerializationError(e.to_string()))?;

        // 追加到文件
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| ComplianceError::StorageError(format!("open file: {}", e)))?;

        writeln!(file, "{}", json)
            .map_err(|e| ComplianceError::StorageError(format!("write event: {}", e)))?;

        Ok(())
    }

    /// 加载指定日期的审计事件
    pub fn load_events(&self, date: NaiveDate) -> ComplianceResult<Vec<AuditEvent>> {
        let path = self.audit_log_path(date);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| ComplianceError::StorageError(format!("read file: {}", e)))?;

        let mut events = Vec::new();
        for line in content.lines() {
            if !line.trim().is_empty() {
                let event: AuditEvent = serde_json::from_str(line)
                    .map_err(|e| ComplianceError::SerializationError(e.to_string()))?;
                events.push(event);
            }
        }

        Ok(events)
    }

    /// 获取审计日志文件路径
    fn audit_log_path(&self, date: NaiveDate) -> PathBuf {
        self.base_path
            .join("audit_logs")
            .join(format!("{}.jsonl", date.format("%Y-%m-%d")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AuditEventType;
    use chrono::Utc;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_event() -> AuditEvent {
        AuditEvent {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::TradeExecuted,
            actor: "test_strategy".into(),
            action: "test_action".into(),
            resource_type: "trade".into(),
            resource_id: Uuid::new_v4().to_string(),
            details: serde_json::json!({}),
            previous_hash: String::new(),
            event_hash: "test_hash".into(),
            ip_address: None,
            session_id: None,
        }
    }

    #[test]
    fn test_storage_creation() {
        let tmp = TempDir::new().unwrap();
        let _storage = FileStorage::new(tmp.path()).unwrap();

        // 验证目录创建
        assert!(tmp.path().join("audit_logs").exists());
    }

    #[test]
    fn test_save_and_load_event() {
        let tmp = TempDir::new().unwrap();
        let storage = FileStorage::new(tmp.path()).unwrap();

        let event = create_test_event();
        let date = event.timestamp.date_naive();

        // 保存事件
        storage.save_event(&event).unwrap();

        // 加载事件
        let loaded = storage.load_events(date).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].event_id, event.event_id);
        assert_eq!(loaded[0].action, event.action);
    }

    #[test]
    fn test_multiple_events_same_day() {
        let tmp = TempDir::new().unwrap();
        let storage = FileStorage::new(tmp.path()).unwrap();

        let event1 = create_test_event();
        let date = event1.timestamp.date_naive();

        let event2 = AuditEvent {
            event_id: Uuid::new_v4(),
            action: "second_action".into(),
            ..create_test_event()
        };

        // 保存多个事件
        storage.save_event(&event1).unwrap();
        storage.save_event(&event2).unwrap();

        // 加载事件
        let loaded = storage.load_events(date).unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn test_load_empty_date() {
        let tmp = TempDir::new().unwrap();
        let storage = FileStorage::new(tmp.path()).unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let loaded = storage.load_events(date).unwrap();
        assert!(loaded.is_empty());
    }
}
