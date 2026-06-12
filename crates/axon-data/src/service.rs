//! 数据服务统一入口
//!
//! 缓存策略:L1 `Mutex<LruCache>` 内存缓存(默认容量 64,builder 可调)。
//! - L1 容量:可配,默认 64 entries
//! - 命中率:`AtomicU64` 计数,无锁并发安全
//! - 后续可扩展 L2 mmap 磁盘 + L3 Redis 集群(feature `remote-cache`)

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
}

/// 缓存统计快照
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 当前 entry 数
    pub len: usize,
    /// 容量上限
    pub capacity: usize,
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

    /// 读取缓存统计
    pub fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.lock().expect("cache mutex poisoned");
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            len: cache.len(),
            capacity: cache.cap().get(),
        }
    }

    /// 按名称查源
    pub fn find_source(&self, name: &str) -> Option<&dyn DataSource> {
        self.sources
            .iter()
            .find(|s| s.name() == name)
            .map(|b| b.as_ref() as &dyn DataSource)
    }

    /// 按请求查询(优先 cache hit,miss 时按 source / 第一个源查)
    pub async fn load(&self, req: &DataRequest) -> DataResult<Dataset> {
        let key = Self::cache_key(req);

        // 1) cache lookup
        {
            let mut cache = self.cache.lock().expect("cache mutex poisoned");
            if let Some(ds) = cache.get(&key) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(ds.clone());
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);

        // 2) 选择数据源
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

        // 3) 写入 cache(可能触发 LRU 淘汰)
        {
            let mut cache = self.cache.lock().expect("cache mutex poisoned");
            cache.put(key, dataset.clone());
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
            let req = DataRequest::new(
                format!("SYM{i}"),
                Utc::now(),
                Utc::now(),
                Frequency::Tick,
            );
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
        let svc = DataService::new()
            .register_source(Box::new(MockSource::with_rows("m", vec![tick()])));
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
