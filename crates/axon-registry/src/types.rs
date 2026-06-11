//! 核心数据类型

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// 语义化版本号（主版本.次版本.补丁）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SemVer {
    /// 主版本
    pub major: u32,
    /// 次版本
    pub minor: u32,
    /// 补丁版本
    pub patch: u32,
}

impl SemVer {
    /// 创建新版本
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// 递增补丁版本
    pub fn bump_patch(&mut self) {
        self.patch += 1;
    }

    /// 递增次版本（重置补丁）
    pub fn bump_minor(&mut self) {
        self.minor += 1;
        self.patch = 0;
    }

    /// 递增主版本（重置次版本和补丁）
    pub fn bump_major(&mut self) {
        self.major += 1;
        self.minor = 0;
        self.patch = 0;
    }

    /// 解析 "1.2.3" 格式
    pub fn parse(s: &str) -> Result<Self, crate::error::RegistryError> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(crate::error::RegistryError::InvalidVersion(s.to_string()));
        }
        let major = parts[0]
            .parse()
            .map_err(|_| crate::error::RegistryError::InvalidVersion(s.to_string()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| crate::error::RegistryError::InvalidVersion(s.to_string()))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| crate::error::RegistryError::InvalidVersion(s.to_string()))?;
        Ok(Self { major, minor, patch })
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 模型阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStage {
    /// 训练完成，待验证
    Staging,
    /// 通过验证，可用于生产
    Production,
    /// 已归档，不再使用
    Archived,
    /// 被回滚标记
    RolledBack,
}

impl std::fmt::Display for ModelStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelStage::Staging => write!(f, "staging"),
            ModelStage::Production => write!(f, "production"),
            ModelStage::Archived => write!(f, "archived"),
            ModelStage::RolledBack => write!(f, "rolled_back"),
        }
    }
}

/// 模型元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelMetadata {
    /// 模型描述
    pub description: String,
    /// 训练参数
    pub hyperparameters: HashMap<String, serde_json::Value>,
    /// 性能指标
    pub metrics: HashMap<String, f64>,
    /// 训练数据集 hash
    pub dataset_hash: Option<String>,
    /// Git commit hash
    pub git_commit: Option<String>,
    /// 训练耗时（秒）
    pub training_duration_secs: Option<f64>,
    /// 创建时间
    pub created_at: SystemTime,
    /// 作者
    pub author: Option<String>,
    /// 标签（自定义键值对）
    pub tags: HashMap<String, String>,
}

impl Default for ModelMetadata {
    fn default() -> Self {
        Self {
            description: String::new(),
            hyperparameters: HashMap::new(),
            metrics: HashMap::new(),
            dataset_hash: None,
            git_commit: None,
            training_duration_secs: None,
            created_at: SystemTime::now(),
            author: None,
            tags: HashMap::new(),
        }
    }
}

/// 模型版本（注册表中的完整记录）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVersion {
    /// 模型名称
    pub name: String,
    /// 版本号
    pub version: SemVer,
    /// 当前阶段
    pub stage: ModelStage,
    /// 元数据
    pub metadata: ModelMetadata,
    /// 模型签名
    pub signature: Option<crate::signature::ModelSignature>,
    /// 存储位置（URI 或路径）
    pub storage_uri: String,
    /// 产物大小（字节）
    pub artifact_size_bytes: u64,
    /// 产物内容 hash（SHA-256）
    pub artifact_hash: String,
}

/// 存储后端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StorageBackend {
    /// 本地文件系统
    Local {
        /// 基础目录
        base_dir: PathBuf,
    },
}

impl Default for StorageBackend {
    fn default() -> Self {
        Self::Local {
            base_dir: PathBuf::from("./models"),
        }
    }
}

/// 上传结果
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// 目标 key
    pub key: String,
    /// 字节数
    pub size_bytes: u64,
    /// 内容 hash
    pub content_hash: String,
}

/// 存储对象元数据
#[derive(Debug, Clone)]
pub struct StorageObject {
    /// 对象 key
    pub key: String,
    /// 字节数
    pub size_bytes: u64,
    /// 最后修改时间
    pub last_modified: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_new_and_display() {
        let v = SemVer::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_semver_parse_ok() {
        let v = SemVer::parse("2.3.4").unwrap();
        assert_eq!(v, SemVer::new(2, 3, 4));
    }

    #[test]
    fn test_semver_parse_invalid_format() {
        let r = SemVer::parse("1.2");
        assert!(r.is_err());
    }

    #[test]
    fn test_semver_parse_invalid_chars() {
        let r = SemVer::parse("a.b.c");
        assert!(r.is_err());
    }

    #[test]
    fn test_semver_bump_patch() {
        let mut v = SemVer::new(1, 0, 0);
        v.bump_patch();
        assert_eq!(v, SemVer::new(1, 0, 1));
    }

    #[test]
    fn test_semver_bump_minor_resets_patch() {
        let mut v = SemVer::new(1, 2, 5);
        v.bump_minor();
        assert_eq!(v, SemVer::new(1, 3, 0));
    }

    #[test]
    fn test_semver_bump_major_resets_minor_patch() {
        let mut v = SemVer::new(1, 2, 3);
        v.bump_major();
        assert_eq!(v, SemVer::new(2, 0, 0));
    }

    #[test]
    fn test_semver_ordering() {
        let a = SemVer::new(1, 0, 0);
        let b = SemVer::new(1, 0, 1);
        let c = SemVer::new(1, 1, 0);
        let d = SemVer::new(2, 0, 0);
        assert!(a < b);
        assert!(b < c);
        assert!(c < d);
    }

    #[test]
    fn test_model_stage_display() {
        assert_eq!(ModelStage::Staging.to_string(), "staging");
        assert_eq!(ModelStage::Production.to_string(), "production");
        assert_eq!(ModelStage::Archived.to_string(), "archived");
        assert_eq!(ModelStage::RolledBack.to_string(), "rolled_back");
    }

    #[test]
    fn test_model_metadata_default() {
        let m = ModelMetadata::default();
        assert!(m.description.is_empty());
        assert!(m.hyperparameters.is_empty());
        assert!(m.dataset_hash.is_none());
    }

    #[test]
    fn test_storage_backend_default() {
        match StorageBackend::default() {
            StorageBackend::Local { base_dir } => {
                assert_eq!(base_dir, PathBuf::from("./models"));
            }
        }
    }
}
