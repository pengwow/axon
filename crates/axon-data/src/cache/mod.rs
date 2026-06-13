//! L2 mmap 共享缓存模块
//!
//! 提供跨进程共享的 Arrow IPC 数据缓存，支持 LRU 淘汰策略。
//!
//! # 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      DataService                            │
//! │  ┌──────────────┐    ┌──────────────┐                      │
//! │  │   L1 Cache   │    │   L2 Cache   │                      │
//! │  │  (LRU 内存)  │    │ (mmap 共享)  │                      │
//! │  └──────┬───────┘    └──────┬───────┘                      │
//! │         │                   │                               │
//! │         ▼                   ▼                               │
//! │  ┌────────────────────────────────────────────────────────┐ │
//! │  │                   Unified API                          │ │
//! │  │  load() / stream() / cache_stats() / cache_control()   │ │
//! │  └────────────────────────────────────────────────────────┘ │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 使用方式
//!
//! ```rust,ignore
//! use axon_data::cache::{MmapCache, MmapCacheConfig};
//!
//! // 创建配置
//! let config = MmapCacheConfig::new(1024 * 1024 * 100, "/tmp/axon_cache");
//!
//! // 创建缓存
//! let mut cache = MmapCache::new(config).unwrap();
//!
//! // 存入数据
//! cache.put("key", &dataset).unwrap();
//!
//! // 读取数据（非零拷贝）
//! let data = cache.get("key");
//!
//! // 零拷贝读取（性能更高，<10µs）
//! let data = cache.get_zero_copy("key");
//! ```

#[cfg(feature = "mmap-cache")]
pub mod shared_memory;

#[cfg(feature = "mmap-cache")]
pub mod mmap;

#[cfg(feature = "mmap-cache")]
pub use mmap::{CachedDataset, MmapCache, MmapCacheConfig};

#[cfg(feature = "mmap-cache")]
pub use shared_memory::SharedMemoryPool;
