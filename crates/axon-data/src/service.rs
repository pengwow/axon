//! 数据服务统一入口
//!
//! 缓存策略:
//! - L1 `Mutex<LruCache>` 内存缓存(默认容量 64,builder 可调)
//! - L2 mmap 共享缓存(feature-gated: mmap-cache)
//!
//! 命中率:`AtomicU64` 计数,无锁并发安全

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lru::LruCache;

use crate::dataset::Dataset;
use crate::error::{DataError, DataResult};
use crate::traits::DataSource;
use crate::types::DataRequest;

/// 数据服务
pub struct DataService {
    sources: Vec<Box<dyn DataSource>>,
    /// L1 LRU 缓存(`Mutex` 保护 LruCache 的内部可变性)
    cache: Mutex<LruCache<u64, Dataset>>,
    /// 缓存容量
    capacity: NonZeroUsize,
    /// 缓存命中次数
    hits: Arc<AtomicU64>,
    /// 缓存未命中次数
    misses: Arc<AtomicU64>,
    /// L2 mmap 共享缓存
    #[cfg(feature = "mmap-cache")]
    mmap_cache: Option<Mutex<crate::cache::MmapCache>>,
    /// L2 缓存命中次数
    #[cfg(feature = "mmap-cache")]
    mmap_hits: Arc<AtomicU64>,
}

/// 缓存统计快照
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// L1 命中次数
    pub hits: u64,
    /// L2 命中次数
    pub l2_hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// L1 当前 entry 数
    pub len: usize,
    /// L1 容量上限
    pub capacity: usize,
    /// L2 当前使用量（字节）
    pub l2_size: usize,
    /// L2 容量上限（字节）
    pub l2_capacity: usize,
}

impl DataService {
    /// 构造空数据服务(默认 LRU 缓存容量 64)
    ///
    /// # Examples
    ///
    /// ```
    /// use axon_data::{DataService, DataRequest, Frequency};
    /// use axon_data::sources::MockSource;
    /// use chrono::Utc;
    ///
    /// let svc = DataService::new()
    ///     .register_source(Box::new(MockSource::empty()));
    /// let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
    /// let ds = futures::executor::block_on(svc.load(&req)).unwrap();
    /// assert_eq!(ds.len(), 0);
    /// ```
    pub fn new() -> Self {
        let cap = NonZeroUsize::new(64).expect("64 is non-zero");
        Self {
            sources: Vec::new(),
            cache: Mutex::new(LruCache::new(cap)),
            capacity: cap,
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            #[cfg(feature = "mmap-cache")]
            mmap_cache: None,
            #[cfg(feature = "mmap-cache")]
            mmap_hits: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 注册数据源(builder 风格)
    ///
    /// # Examples
    ///
    /// ```
    /// use axon_data::DataService;
    /// use axon_data::sources::MockSource;
    ///
    /// let svc = DataService::new()
    ///     .register_source(Box::new(MockSource::empty()));
    /// assert_eq!(svc.find_source("mock").map(|s| s.name()), Some("mock"));
    /// ```
    pub fn register_source(mut self, source: Box<dyn DataSource>) -> Self {
        self.sources.push(source);
        self
    }

    /// 调整 LRU 容量(builder 风格,需在 `new` 后、`load` 前调用)
    ///
    /// # Examples
    ///
    /// ```
    /// use axon_data::DataService;
    /// use std::num::NonZeroUsize;
    ///
    /// let svc = DataService::new()
    ///     .with_cache_capacity(NonZeroUsize::new(128).unwrap());
    /// assert_eq!(svc.cache_stats().capacity, 128);
    /// ```
    pub fn with_cache_capacity(mut self, cap: NonZeroUsize) -> Self {
        self.capacity = cap;
        // 重建缓存以应用新容量(简单做法:新空 LRU 替换;旧 entries 丢弃)
        self.cache = Mutex::new(LruCache::new(cap));
        self
    }

    /// 启用 L2 mmap 缓存(builder 风格)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use axon_data::DataService;
    /// use axon_data::cache::MmapCacheConfig;
    ///
    /// let svc = DataService::new()
    ///     .with_mmap_cache(MmapCacheConfig::new(1024 * 1024 * 100, "/tmp/axon_cache"))
    ///     .unwrap();
    /// ```
    #[cfg(feature = "mmap-cache")]
    pub fn with_mmap_cache(mut self, config: crate::cache::MmapCacheConfig) -> DataResult<Self> {
        let cache = crate::cache::MmapCache::new(config)?;
        self.mmap_cache = Some(Mutex::new(cache));
        self.mmap_hits = Arc::new(AtomicU64::new(0));
        Ok(self)
    }

    /// 读取缓存统计
    pub fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.lock().expect("cache mutex poisoned");

        #[cfg(feature = "mmap-cache")]
        let (l2_size, l2_capacity, l2_hits) = if let Some(ref cache) = self.mmap_cache {
            if let Ok(cache) = cache.lock() {
                (
                    cache.used(),
                    cache.capacity(),
                    self.mmap_hits.load(Ordering::Relaxed),
                )
            } else {
                (0, 0, 0)
            }
        } else {
            (0, 0, 0)
        };

        #[cfg(not(feature = "mmap-cache"))]
        let (l2_size, l2_capacity, l2_hits) = (0, 0, 0);

        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            l2_hits,
            misses: self.misses.load(Ordering::Relaxed),
            len: cache.len(),
            capacity: cache.cap().get(),
            l2_size,
            l2_capacity,
        }
    }

    /// 按名称查源
    pub fn find_source(&self, name: &str) -> Option<&dyn DataSource> {
        self.sources
            .iter()
            .find(|s| s.name() == name)
            .map(|b| b.as_ref() as &dyn DataSource)
    }

    /// 按请求查询(优先 L1 → L2 → 数据源)
    pub async fn load(&self, req: &DataRequest) -> DataResult<Dataset> {
        let key = Self::cache_key(req);

        // 1) L1 cache lookup
        {
            let mut cache = self.cache.lock().expect("cache mutex poisoned");
            if let Some(ds) = cache.get(&key) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(ds.clone());
            }
        }

        // 2) L2 cache lookup (if enabled)
        #[cfg(feature = "mmap-cache")]
        if let Some(ref cache) = self.mmap_cache
            && let Ok(mut cache) = cache.lock()
        {
            let l2_key = crate::cache::MmapCache::cache_key(
                req.source.as_deref().unwrap_or("unknown"),
                &req.symbol,
                req.frequency.as_str(),
            );
            if let Some(ds) = cache.get(&l2_key) {
                self.mmap_hits.fetch_add(1, Ordering::Relaxed);
                // 写入 L1
                let mut l1_cache = self.cache.lock().expect("cache mutex poisoned");
                l1_cache.put(key, ds.clone());
                return Ok(ds);
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);

        // 3) 选择数据源
        let source: &dyn DataSource = match &req.source {
            Some(name) => self
                .find_source(name)
                .ok_or_else(|| DataError::SourceNotFound(name.clone()))?,
            None => self
                .sources
                .first()
                .map(|b| b.as_ref() as &dyn DataSource)
                .ok_or_else(|| DataError::SourceNotFound("<no source registered>".into()))?,
        };

        let dataset = source.query(req).await?;

        // 4) 写入 L1 cache(可能触发 LRU 淘汰)
        {
            let mut cache = self.cache.lock().expect("cache mutex poisoned");
            cache.put(key, dataset.clone());
        }

        // 5) 写入 L2 cache (if enabled)
        #[cfg(feature = "mmap-cache")]
        if let Some(ref cache) = self.mmap_cache
            && let Ok(mut cache) = cache.lock()
        {
            let l2_key = crate::cache::MmapCache::cache_key(
                req.source.as_deref().unwrap_or("unknown"),
                &req.symbol,
                req.frequency.as_str(),
            );
            let _ = cache.put(&l2_key, &dataset);
        }

        Ok(dataset)
    }

    fn cache_key(req: &DataRequest) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        req.hash(&mut h);
        h.finish()
    }
}

impl Default for DataService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::MockSource;
    use crate::types::Frequency;
    use axon_core::market::{Side, Tick};
    use axon_core::time::Timestamp;
    use axon_core::types::{Price, Quantity};
    use chrono::Utc;

    fn tick() -> Tick {
        Tick::new(
            Timestamp::from_nanos(0),
            Price::from_f64(1.0),
            Quantity::from(1.0),
            Side::Buy,
        )
    }

    #[tokio::test]
    async fn load_with_no_source_returns_error() {
        let svc = DataService::new();
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let res = svc.load(&req).await;
        assert!(matches!(res, Err(DataError::SourceNotFound(_))));
    }

    #[tokio::test]
    async fn load_with_mock_returns_dataset() {
        let svc = DataService::new()
            .register_source(Box::new(MockSource::with_rows("mock", vec![tick()])));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = svc.load(&req).await.unwrap();
        assert_eq!(ds.len(), 1);
    }

    #[tokio::test]
    async fn cache_hit_avoids_duplicate_query() {
        let svc = DataService::new()
            .register_source(Box::new(MockSource::with_rows("mock", vec![tick()])));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds1 = svc.load(&req).await.unwrap();
        let ds2 = svc.load(&req).await.unwrap();
        assert_eq!(ds1.checksum, ds2.checksum);
    }

    #[tokio::test]
    async fn lru_evicts_oldest_when_capacity_exceeded() {
        // 容量 2,插入 3 个不同 key,触发淘汰
        let svc = DataService::new()
            .with_cache_capacity(NonZeroUsize::new(2).unwrap())
            .register_source(Box::new(MockSource::with_rows("m", vec![tick()])));
        for i in 0..3 {
            let req = DataRequest::new(format!("SYM{i}"), Utc::now(), Utc::now(), Frequency::Tick);
            let _ = svc.load(&req).await.unwrap();
        }
        let stats = svc.cache_stats();
        assert_eq!(stats.len, 2);
        assert_eq!(stats.capacity, 2);
        // 3 次 load 应全是 miss(都不同 key)
        assert_eq!(stats.misses, 3);
        assert_eq!(stats.hits, 0);
    }

    #[tokio::test]
    async fn cache_hit_increments_hits_counter() {
        let svc =
            DataService::new().register_source(Box::new(MockSource::with_rows("m", vec![tick()])));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        svc.load(&req).await.unwrap(); // miss
        svc.load(&req).await.unwrap(); // hit
        svc.load(&req).await.unwrap(); // hit
        let stats = svc.cache_stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 2);
    }

    #[tokio::test]
    async fn default_cache_capacity_is_64() {
        let svc = DataService::new();
        let stats = svc.cache_stats();
        assert_eq!(stats.capacity, 64);
        assert_eq!(stats.len, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }
}
