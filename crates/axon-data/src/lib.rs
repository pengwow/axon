//! AXON 数据服务
//!
//! 提供统一的市场数据接入、特征计算与缓存能力（Phase 3 P2 骨架）。
//!
//! ## 模块
//!
//! - [`types`]:核心数据类型 `DataRequest` / `Frequency` / `Dataset`
//! - [`error`]:统一错误类型 `DataError`
//! - [`dataset`]:数据集合（内存表示 + 行式迭代）
//! - [`traits`]:数据源抽象 `DataSource` trait
//! - [`sources`]:具体数据源实现（默认仅暴露 `MockSource`,csv/ws feature-gated）
//! - [`pipeline`][]:特征管道（归一化、滑动窗口）骨架
//!
//! ## Feature flag
//!
//! - `csv-source`:启用 `CsvSource`
//! - `ws-source`:启用 `WebSocketSource`(默认关闭)

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod bar;
pub mod dataset;
pub mod error;
pub mod ipc;
pub mod pipeline;
pub mod sources;
pub mod traits;
pub mod types;

/// L2 mmap 共享缓存模块（feature-gated: mmap-cache）
///
/// # Safety
///
/// 本模块使用 `memmap2` 的 unsafe API 进行内存映射。
/// 这是必要的，因为：
/// 1. 文件在映射期间可能被其他进程修改
/// 2. 文件可能被截断
///
/// 在我们的使用场景中，这是安全的，因为：
/// 1. 我们在写入后立即映射，不会在映射期间修改文件
/// 2. 我们控制文件的生命周期
/// 3. 我们使用元数据头验证数据完整性
#[cfg(feature = "mmap-cache")]
#[allow(unsafe_code)]
pub mod cache;

// 内部模块
mod service;
// Property-based fuzz tests(仅测试时编译)
#[cfg(test)]
mod fuzz;

pub use dataset::Dataset;
pub use error::DataError;
pub use pipeline::{FeatureMatrix, FeaturePipeline, Normalizer, ZScoreNormalizer};
pub use service::{CacheStats, DataService};
pub use sources::MockSource;
pub use traits::DataSource;
pub use types::{DataRequest, Frequency};
