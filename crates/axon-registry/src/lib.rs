//! AXON 模型注册表
//!
//! 版本管理 + 阶段生命周期 + 多后端存储 + 元数据签名。

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod error;
pub mod filter;
pub mod registry;
pub mod signature;
pub mod types;

pub mod storage;

#[cfg(feature = "python")]
pub mod python;

pub use error::{RegistryError, RegistryResult};
pub use filter::VersionFilter;
pub use registry::ModelRegistry;
pub use signature::{DataType, ModelSignature, SignatureField};
pub use storage::{LocalStorage, StorageBackendTrait};
pub use types::{ModelMetadata, ModelStage, ModelVersion, SemVer, StorageBackend, StorageObject, UploadResult};
