//! 数据源 trait 抽象
//!
//! 所有数据源(Csv / Parquet / WebSocket / Mock / Replay)统一实现此 trait,
//! 通过 `Box<dyn DataSource>` 注入到 [`DataService`]。

use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;

use crate::dataset::Dataset;
use crate::error::DataResult;
use crate::types::{DataRequest, SchemaField};

/// 数据源抽象
#[async_trait]
pub trait DataSource: Send + Sync {
    /// 数据源名称(唯一,用于按名查找)
    fn name(&self) -> &str;

    /// 字段 schema 描述
    fn schema(&self) -> &[SchemaField];

    /// 同步查询:返回完整数据集
    async fn query(&self, req: &DataRequest) -> DataResult<Dataset>;

    /// 流式订阅:返回 `Result<Tick>` 的 stream
    ///
    /// 注意:此 API 必须在 `ws-source` 或 `csv-source` feature 启用时实现;
    /// Mock 默认返回空 stream。
    async fn stream(
        &self,
        req: &DataRequest,
    ) -> DataResult<Pin<Box<dyn Stream<Item = DataResult<axon_core::market::Tick>> + Send>>>;
}
