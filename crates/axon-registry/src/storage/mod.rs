//! 存储后端实现

pub mod local;

use std::any::Any;
use std::path::Path;

use async_trait::async_trait;

use crate::error::RegistryResult;
use crate::types::{StorageObject, UploadResult};

pub use local::LocalStorage;

/// 存储后端抽象
///
/// 设计为 async trait 以支持 S3/HTTP 等网络后端；
/// LocalStorage 在内部用 `tokio::task::spawn_blocking` 包装阻塞 IO。
#[async_trait]
pub trait StorageBackendTrait: Send + Sync {
    /// 上传模型产物
    async fn upload(&self, source: &Path, dest_key: &str) -> RegistryResult<UploadResult>;

    /// 下载模型产物
    async fn download(&self, source_key: &str, dest: &Path) -> RegistryResult<()>;

    /// 列出指定前缀下的所有文件
    async fn list(&self, prefix: &str) -> RegistryResult<Vec<StorageObject>>;

    /// 删除文件
    async fn delete(&self, key: &str) -> RegistryResult<()>;

    /// 检查文件是否存在
    async fn exists(&self, key: &str) -> RegistryResult<bool>;

    /// 访问内部类型（用于 downcast 推断 persist_dir）
    fn as_any(&self) -> &dyn Any;
}
