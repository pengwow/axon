//! Mock 数据源(测试用)

use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::dataset::Dataset;
use crate::error::DataResult;
use crate::traits::DataSource;
use crate::types::{DataRequest, SchemaField};

use axon_core::market::{Side, Tick};
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};

/// Mock 数据源(默认实现,在 `mod.rs` 中已使用)
pub struct MockSource {
    name: String,
    /// 行数组(对内可见,供 fuzz.rs 不变量测试访问;对外通过 `query`/`iter` 暴露)
    pub(crate) rows: Vec<Tick>,
}

impl MockSource {
    /// 构造空 Mock
    pub fn empty() -> Self {
        Self {
            name: "mock".into(),
            rows: Vec::new(),
        }
    }

    /// 构造带预置数据的 Mock
    pub fn with_rows(name: impl Into<String>, rows: Vec<Tick>) -> Self {
        Self {
            name: name.into(),
            rows,
        }
    }

    /// 时间序列生成器
    ///
    /// 生成 `count` 个 tick,价格按 `price_fn(i)` 计算,
    /// 时间按 `nanos_per_step` 间隔均匀递增(从 0 开始)。
    ///
    /// # Examples
    ///
    /// ```
    /// use axon_data::sources::MockSource;
    /// let mock = MockSource::with_tick_series("btc", 100, 1_000_000_000, |i| 100.0 + i as f64);
    /// ```
    pub fn with_tick_series<F>(
        name: impl Into<String>,
        count: usize,
        nanos_per_step: i64,
        price_fn: F,
    ) -> Self
    where
        F: Fn(usize) -> f64,
    {
        let mut rows = Vec::with_capacity(count);
        for i in 0..count {
            rows.push(Tick::new(
                Timestamp::from_nanos(i as i64 * nanos_per_step),
                Price::from_f64(price_fn(i)),
                Quantity::from_f64(1.0),
                Side::Buy,
            ));
        }
        Self {
            name: name.into(),
            rows,
        }
    }
}

#[async_trait]
impl DataSource for MockSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn schema(&self) -> &[SchemaField] {
        &[]
    }

    async fn query(&self, req: &DataRequest) -> DataResult<Dataset> {
        Ok(Dataset::new(
            self.rows.clone(),
            vec![],
            self.name.clone(),
            req.clone(),
        ))
    }

    async fn stream(
        &self,
        _req: &DataRequest,
    ) -> DataResult<Pin<Box<dyn Stream<Item = DataResult<Tick>> + Send>>>
    {
        // Mock:用自定义 EmptyStream(避免依赖 `futures-util` / `tokio-stream`)
        Ok(Box::pin(EmptyStream::<DataResult<Tick>>::new()))
    }
}

/// 永不产出的空 stream(避免依赖 `futures-util` / `tokio-stream`)
pub struct EmptyStream<T>(std::marker::PhantomData<T>);

impl<T> EmptyStream<T> {
    /// 构造一个空 stream
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T> Stream for EmptyStream<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DataRequest, Frequency};
    use chrono::Utc;

    fn make_test_req() -> DataRequest {
        DataRequest::new(
            "MOCK",
            Utc::now() - chrono::Duration::days(1),
            Utc::now(),
            Frequency::Tick,
        )
    }

    #[test]
    fn with_tick_series_generates_correct_count() {
        let mock = MockSource::with_tick_series("btc", 5, 1_000_000_000, |i| 100.0 + i as f64);
        let ds = futures::executor::block_on(mock.query(&make_test_req())).unwrap();
        assert_eq!(ds.len(), 5);
    }

    #[test]
    fn with_tick_series_count_zero_yields_empty() {
        let mock = MockSource::with_tick_series("x", 0, 1, |_| 0.0);
        let ds = futures::executor::block_on(mock.query(&make_test_req())).unwrap();
        assert!(ds.is_empty());
    }

    #[test]
    fn with_tick_series_timestamps_advance_uniformly() {
        let mock = MockSource::with_tick_series("x", 3, 100, |_| 50.0);
        let ds = futures::executor::block_on(mock.query(&make_test_req())).unwrap();
        let ts: Vec<i64> = ds.iter().map(|t| t.timestamp.nanos).collect();
        assert_eq!(ts, vec![0, 100, 200]);
    }

    #[test]
    fn with_tick_series_prices_follow_fn() {
        let mock = MockSource::with_tick_series("x", 4, 1, |i| (i * 2) as f64);
        let ds = futures::executor::block_on(mock.query(&make_test_req())).unwrap();
        let prices: Vec<f64> = ds.iter().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![0.0, 2.0, 4.0, 6.0]);
    }
}
