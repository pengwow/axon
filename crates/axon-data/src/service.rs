//! 数据服务统一入口(骨架)
//!
//! 后续可扩展为:
//! - L1 `DashMap` 内存缓存
//! - L2 LRU + mmap 磁盘缓存
//! - L3 Redis 集群(feature `remote-cache`)
//! - 多源融合 + 失败回退

use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;

use crate::dataset::Dataset;
use crate::error::{DataError, DataResult};
use crate::traits::DataSource;
use crate::types::DataRequest;

/// 数据服务
pub struct DataService {
    sources: Vec<Box<dyn DataSource>>,
    /// L1 缓存(请求 hash -> dataset)
    cache: Arc<DashMap<u64, Arc<RwLock<Dataset>>>>,
}

impl DataService {
    /// 构造空服务
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            cache: Arc::new(DashMap::new()),
        }
    }

    /// 注册数据源
    pub fn register_source(mut self, source: Box<dyn DataSource>) -> Self {
        self.sources.push(source);
        self
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
        if let Some(entry) = self.cache.get(&key) {
            return Ok(entry.read().clone());
        }

        // 选择数据源
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
        self.cache.insert(key, Arc::new(RwLock::new(dataset.clone())));
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

    #[tokio::test]
    async fn load_with_no_source_returns_error() {
        let svc = DataService::new();
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let res = svc.load(&req).await;
        assert!(matches!(res, Err(DataError::SourceNotFound(_))));
    }

    #[tokio::test]
    async fn load_with_mock_returns_dataset() {
        let tick = Tick::new(
            Timestamp::from_nanos(0),
            Price::from_f64(1.0),
            Quantity::from(1.0),
            Side::Buy,
        );
        let svc = DataService::new()
            .register_source(Box::new(MockSource::with_rows("mock", vec![tick])));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = svc.load(&req).await.unwrap();
        assert_eq!(ds.len(), 1);
    }

    #[tokio::test]
    async fn cache_hit_avoids_duplicate_query() {
        let tick = Tick::new(
            Timestamp::from_nanos(0),
            Price::from_f64(1.0),
            Quantity::from(1.0),
            Side::Buy,
        );
        let svc = DataService::new()
            .register_source(Box::new(MockSource::with_rows("mock", vec![tick])));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds1 = svc.load(&req).await.unwrap();
        let ds2 = svc.load(&req).await.unwrap();
        // 同一请求命中 cache,checksum 应一致
        assert_eq!(ds1.checksum, ds2.checksum);
    }
}
