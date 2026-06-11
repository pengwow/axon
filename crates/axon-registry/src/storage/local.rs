//! 本地文件系统存储

use std::any::Any;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::task;

use crate::error::{RegistryError, RegistryResult};
use crate::storage::StorageBackendTrait;
use crate::types::{StorageObject, UploadResult};

/// 本地文件系统存储
pub struct LocalStorage {
    base_dir: PathBuf,
}

impl LocalStorage {
    /// 创建本地存储
    pub fn new(base_dir: PathBuf) -> Result<Self, RegistryError> {
        std::fs::create_dir_all(&base_dir).map_err(|e| RegistryError::Io(e.to_string()))?;
        Ok(Self { base_dir })
    }

    /// 获取基础目录
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    fn full_path(&self, key: &str) -> PathBuf {
        self.base_dir.join(key)
    }

    fn compute_hash(path: &Path) -> Result<String, RegistryError> {
        let data = std::fs::read(path).map_err(|e| RegistryError::Io(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(format!("{:x}", hasher.finalize()))
    }
}

#[async_trait]
impl StorageBackendTrait for LocalStorage {
    async fn upload(&self, source: &Path, dest_key: &str) -> RegistryResult<UploadResult> {
        let dest = self.full_path(dest_key);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RegistryError::Io(e.to_string()))?;
        }
        let metadata = std::fs::metadata(source)
            .map_err(|e| RegistryError::StorageError(e.to_string()))?;
        let size = metadata.len();

        // 异步复制文件
        let src = source.to_path_buf();
        let dst = dest.clone();
        task::spawn_blocking(move || std::fs::copy(&src, &dst))
            .await
            .map_err(|e| RegistryError::StorageError(e.to_string()))?
            .map_err(|e| RegistryError::StorageError(e.to_string()))?;

        let content_hash = Self::compute_hash(&dest)?;

        Ok(UploadResult {
            key: dest_key.to_string(),
            size_bytes: size,
            content_hash,
        })
    }

    async fn download(&self, source_key: &str, dest: &Path) -> RegistryResult<()> {
        let src = self.full_path(source_key);
        if !src.exists() {
            return Err(RegistryError::ArtifactNotFound(source_key.to_string()));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RegistryError::Io(e.to_string()))?;
        }
        let src_clone = src.clone();
        let dest_clone = dest.to_path_buf();
        task::spawn_blocking(move || std::fs::copy(&src_clone, &dest_clone))
            .await
            .map_err(|e| RegistryError::StorageError(e.to_string()))?
            .map_err(|e| RegistryError::StorageError(e.to_string()))?;
        Ok(())
    }

    async fn list(&self, prefix: &str) -> RegistryResult<Vec<StorageObject>> {
        let base = self.base_dir.clone();
        let prefix_owned = prefix.to_string();
        let objects: Vec<StorageObject> = task::spawn_blocking(move || {
            let mut objects = Vec::new();
            let dir_path = base.join(&prefix_owned);
            if dir_path.exists() {
                for entry in std::fs::read_dir(&dir_path)
                    .map_err(|e| RegistryError::StorageError(e.to_string()))?
                {
                    let entry = entry.map_err(|e| RegistryError::StorageError(e.to_string()))?;
                    let metadata = entry
                        .metadata()
                        .map_err(|e| RegistryError::StorageError(e.to_string()))?;
                    let key = entry.file_name().to_string_lossy().to_string();
                    objects.push(StorageObject {
                        key: format!("{prefix_owned}{key}"),
                        size_bytes: metadata.len(),
                        last_modified: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
                    });
                }
            }
            Ok::<Vec<StorageObject>, RegistryError>(objects)
        })
        .await
        .map_err(|e| RegistryError::StorageError(e.to_string()))??;
        Ok(objects)
    }

    async fn delete(&self, key: &str) -> RegistryResult<()> {
        let path = self.full_path(key);
        let path_owned = path.clone();
        task::spawn_blocking(move || std::fs::remove_file(&path_owned))
            .await
            .map_err(|e| RegistryError::StorageError(e.to_string()))?
            .map_err(|e| RegistryError::StorageError(e.to_string()))?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> RegistryResult<bool> {
        let path = self.full_path(key);
        Ok(path.exists())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_upload_and_download() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf()).unwrap();

        // 准备源文件
        let src = tmp.path().join("src.bin");
        std::fs::write(&src, b"hello world").unwrap();

        // 上传
        let result = storage
            .upload(&src, "models/test/v1/model.bin")
            .await
            .unwrap();
        assert_eq!(result.size_bytes, 11);
        assert!(!result.content_hash.is_empty());

        // 下载
        let dest = tmp.path().join("downloaded.bin");
        storage
            .download("models/test/v1/model.bin", &dest)
            .await
            .unwrap();
        let content = std::fs::read(&dest).unwrap();
        assert_eq!(content, b"hello world");
    }

    #[tokio::test]
    async fn test_list_objects() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf()).unwrap();

        let src = tmp.path().join("src.bin");
        std::fs::write(&src, b"data").unwrap();

        storage.upload(&src, "models/v1/a.bin").await.unwrap();
        storage.upload(&src, "models/v1/b.bin").await.unwrap();

        let objects = storage.list("models/v1/").await.unwrap();
        assert_eq!(objects.len(), 2);
    }

    #[tokio::test]
    async fn test_exists_and_delete() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf()).unwrap();

        let src = tmp.path().join("src.bin");
        std::fs::write(&src, b"data").unwrap();
        storage.upload(&src, "key1").await.unwrap();

        assert!(storage.exists("key1").await.unwrap());
        storage.delete("key1").await.unwrap();
        assert!(!storage.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_sha256_hash_consistent() {
        let tmp = TempDir::new().unwrap();
        let storage = LocalStorage::new(tmp.path().to_path_buf()).unwrap();

        let src = tmp.path().join("src.bin");
        std::fs::write(&src, b"hello world").unwrap();

        let r1 = storage.upload(&src, "a.bin").await.unwrap();
        // 重复上传得到相同 hash
        std::fs::write(&src, b"hello world").unwrap();
        let r2 = storage.upload(&src, "b.bin").await.unwrap();
        assert_eq!(r1.content_hash, r2.content_hash);
    }
}
