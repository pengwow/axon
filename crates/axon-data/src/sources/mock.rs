//! Mock 数据源(测试用)

use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::dataset::Dataset;
use crate::error::DataResult;
use crate::traits::DataSource;
use crate::types::{DataRequest, SchemaField};

use axon_core::market::Tick;

/// Mock 数据源(默认实现,在 `mod.rs` 中已使用)
pub struct MockSource {
    name: String,
    rows: Vec<Tick>,
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
