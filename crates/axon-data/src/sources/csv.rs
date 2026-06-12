//! CSV 数据源(需 `csv-source` feature)
//!
//! 骨架实现:仅支持最简格式(列序固定:`timestamp,price,quantity,side`)。
//! 后续可扩展为自动 schema 推断 + 灵活列映射。

use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;

use crate::dataset::Dataset;
use crate::error::{DataError, DataResult};
use crate::traits::DataSource;
use crate::types::{DataRequest, Frequency, SchemaField};

use axon_core::market::{Side, Tick};
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};

/// CSV 数据源
pub struct CsvSource {
    name: String,
    path: String,
}

impl CsvSource {
    /// 从文件路径构造
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
        }
    }
}

#[async_trait]
impl DataSource for CsvSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn schema(&self) -> &[SchemaField] {
        // 简化 schema 描述
        const SCHEMA: &[SchemaField] = &[
            SchemaField { name: "timestamp".into(), dtype: crate::types::DataType::Timestamp },
            SchemaField { name: "price".into(), dtype: crate::types::DataType::F64 },
            SchemaField { name: "quantity".into(), dtype: crate::types::DataType::F64 },
            SchemaField { name: "side".into(), dtype: crate::types::DataType::String },
        ];
        SCHEMA
    }

    async fn query(&self, req: &DataRequest) -> DataResult<Dataset> {
        let mut reader = csv::Reader::from_path(&self.path)
            .map_err(|e| DataError::InvalidRequest(format!("open {}: {}", self.path, e)))?;

        let mut rows = Vec::new();
        for (i, record) in reader.records().enumerate() {
            let record = record.map_err(|e| {
                DataError::CorruptData {
                    expected: "valid csv row".into(),
                    actual: format!("line {i}: {e}"),
                }
            })?;

            // 期望 4 列:timestamp(纳秒整数), price, quantity, side(buy/sell)
            if record.len() != 4 {
                return Err(DataError::SchemaMismatch {
                    expected: "4 columns (timestamp,price,quantity,side)".into(),
                    actual: format!("{} columns", record.len()),
                });
            }

            let ts_nanos: i64 = record[0]
                .parse()
                .map_err(|e| DataError::CorruptData {
                    expected: "i64 timestamp".into(),
                    actual: format!("line {i}: {e}"),
                })?;
            let price: f64 = record[1]
                .parse()
                .map_err(|e| DataError::CorruptData {
                    expected: "f64 price".into(),
                    actual: format!("line {i}: {e}"),
                })?;
            let qty: f64 = record[2]
                .parse()
                .map_err(|e| DataError::CorruptData {
                    expected: "f64 quantity".into(),
                    actual: format!("line {i}: {e}"),
                })?;
            let side = match record[3].to_ascii_lowercase().as_str() {
                "buy" | "b" => Side::Buy,
                "sell" | "s" => Side::Sell,
                other => {
                    return Err(DataError::CorruptData {
                        expected: "buy/sell".into(),
                        actual: format!("line {i}: '{other}'"),
                    })
                }
            };

            let timestamp = Timestamp::from_nanos(ts_nanos);
            let price = Price::from_f64(price).map_err(|e| {
                DataError::CorruptData {
                    expected: "valid f64 price".into(),
                    actual: format!("line {i}: {e}"),
                }
            })?;
            let quantity = Quantity::from_f64(qty).map_err(|e| DataError::CorruptData {
                expected: "valid f64 quantity".into(),
                actual: format!("line {i}: {e}"),
            })?;

            rows.push(Tick::new(timestamp, price, quantity, side));
        }

        // 验证 frequency 与数据匹配(简单断言:Tick 频率必须是 Tick)
        if req.frequency != Frequency::Tick && !rows.is_empty() {
            return Err(DataError::InvalidRequest(format!(
                "CsvSource 骨架仅支持 Tick 频率,收到 {:?}",
                req.frequency
            )));
        }

        Ok(Dataset::new(
            rows,
            self.schema().to_vec(),
            self.name.clone(),
            req.clone(),
        ))
    }

    async fn stream(
        &self,
        req: &DataRequest,
    ) -> DataResult<Pin<Box<dyn Stream<Item = DataResult<Tick>> + Send>>> {
        // 简化:流式读整个文件后用 `iter().map(Result::Ok)` 转 stream
        let dataset = self.query(req).await?;
        let stream = futures_core::stream::iter(dataset.rows.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}
