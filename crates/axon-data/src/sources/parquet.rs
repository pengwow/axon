//! Parquet 数据源(feature = "parquet-source")
//!
//! 严格 4 列 schema(按位置):
//! 0: timestamp(int64, 纳秒)
//! 1: price(float64)
//! 2: quantity(float64)
//! 3: side(string) — 接受 "buy"/"sell" 或 "b"/"s"
//!
//! 与 `CsvSource` 行为对齐:
//! - 同样的 `Vec<Tick>` 输出
//! - 同样的 `Frequency::Tick` 约束(其他频率返回 `InvalidRequest`)
//! - 同样的 `Box::leak` 模式用于稳定 schema 切片
//!
//! 加载策略:全量加载(与 `CsvSource` 一致),后续 PR4+ 可升级为流式 row group 读取。

use std::fs::File;
use std::path::PathBuf;
use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;

use arrow::array::{Array, Float64Array, Int64Array, StringArray};
use arrow::datatypes::DataType as ArrowType;
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::dataset::Dataset;
use crate::error::{CsvLocation, DataError, DataResult};
use crate::traits::DataSource;
use crate::types::{DataRequest, Frequency, SchemaField};

use axon_core::market::{Side, Tick};
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};

/// 默认 batch size(每批 Arrow RecordBatch 包含的行数)
const DEFAULT_BATCH_SIZE: usize = 1024;

/// Parquet 数据源
///
/// 读取 `.parquet` 文件,严格校验 4 列 schema(按位置)后,
/// 转换为 `Vec<Tick>` 并通过 `Dataset` 暴露。
pub struct ParquetSource {
    name: String,
    path: PathBuf,
    /// 每次 Arrow RecordBatch 的行数(默认 1024)
    batch_size: usize,
}

impl ParquetSource {
    /// 构造(使用默认 batch_size = 1024)
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }

    /// 自定义 batch size(builder 风格)
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    /// 验证 schema:严格 4 列(int64, f64, f64, utf8)
    fn validate_schema(schema: &std::sync::Arc<arrow::datatypes::Schema>) -> DataResult<()> {
        let fields = schema.fields();
        if fields.len() < 4 {
            return Err(DataError::SchemaMismatch {
                expected: "≥4 columns (timestamp, price, quantity, side)".into(),
                actual: format!("{} columns", fields.len()),
            });
        }
        let expected = [
            ArrowType::Int64,
            ArrowType::Float64,
            ArrowType::Float64,
            ArrowType::Utf8,
        ];
        for (i, exp) in expected.iter().enumerate() {
            if fields[i].data_type() != exp {
                return Err(DataError::SchemaMismatch {
                    expected: format!("column {i} = {exp:?}"),
                    actual: format!("column {i} = {:?}", fields[i].data_type()),
                });
            }
        }
        Ok(())
    }

    /// 稳定 schema 字段(用于 `DataSource::schema`)
    fn schema_fields() -> Vec<SchemaField> {
        vec![
            SchemaField::new("timestamp", crate::types::DataType::Timestamp),
            SchemaField::new("price", crate::types::DataType::F64),
            SchemaField::new("quantity", crate::types::DataType::F64),
            SchemaField::new("side", crate::types::DataType::String),
        ]
    }

    /// 内部同步加载逻辑(在 `spawn_blocking` 中运行)
    fn load_sync(&self, req: &DataRequest) -> DataResult<Dataset> {
        // 1. 打开文件
        let file = File::open(&self.path).map_err(|e| {
            DataError::InvalidRequest(format!("open {}: {}", self.path.display(), e))
        })?;

        // 2. 构造 builder 并校验 schema
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| {
            DataError::CorruptData {
                expected: "valid parquet file".into(),
                actual: format!("{e}"),
                location: Some(CsvLocation {
                    file: self.path.to_string_lossy().into_owned(),
                    line: 0,
                    column: None,
                }),
            }
        })?;
        let schema = builder.schema().clone();
        Self::validate_schema(&schema)?;

        // 3. 构造 batch reader 并迭代
        let batch_reader = builder
            .with_batch_size(self.batch_size)
            .build()
            .map_err(|e| DataError::CorruptData {
                expected: "record batch reader".into(),
                actual: format!("{e}"),
                location: Some(CsvLocation {
                    file: self.path.to_string_lossy().into_owned(),
                    line: 0,
                    column: None,
                }),
            })?;

        let mut rows: Vec<Tick> = Vec::new();
        for batch_result in batch_reader {
            let batch = batch_result.map_err(|e| DataError::CorruptData {
                expected: "valid record batch".into(),
                actual: format!("{e}"),
                location: Some(CsvLocation {
                    file: self.path.to_string_lossy().into_owned(),
                    line: rows.len(),
                    column: None,
                }),
            })?;
            rows.extend(batch_to_ticks(&batch)?);
        }

        // 4. Frequency 校验(与 CsvSource 一致)
        if req.frequency != Frequency::Tick && !rows.is_empty() {
            return Err(DataError::InvalidRequest(format!(
                "ParquetSource 骨架仅支持 Tick 频率,收到 {:?}",
                req.frequency
            )));
        }

        Ok(Dataset::new(
            rows,
            Self::schema_fields(),
            self.name.clone(),
            req.clone(),
        ))
    }
}

/// `RecordBatch` (4 列) → `Vec<Tick>`
///
/// 列下转型失败返回 `CorruptData`(schema 已校验过,理论上不会发生,
/// 但保留防护以防上游 caller 误用)。
fn batch_to_ticks(batch: &RecordBatch) -> DataResult<Vec<Tick>> {
    let ts_col = batch
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| DataError::CorruptData {
            expected: "Int64Array for column 0 (timestamp)".into(),
            actual: format!("got {:?}", batch.column(0).data_type()),
            location: Some(CsvLocation {
                file: "parquet".into(),
                line: 0,
                column: Some("timestamp".into()),
            }),
        })?;
    let price_col = batch
        .column(1)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| DataError::CorruptData {
            expected: "Float64Array for column 1 (price)".into(),
            actual: format!("got {:?}", batch.column(1).data_type()),
            location: Some(CsvLocation {
                file: "parquet".into(),
                line: 0,
                column: Some("price".into()),
            }),
        })?;
    let qty_col = batch
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| DataError::CorruptData {
            expected: "Float64Array for column 2 (quantity)".into(),
            actual: format!("got {:?}", batch.column(2).data_type()),
            location: Some(CsvLocation {
                file: "parquet".into(),
                line: 0,
                column: Some("quantity".into()),
            }),
        })?;
    let side_col = batch
        .column(3)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| DataError::CorruptData {
            expected: "StringArray for column 3 (side)".into(),
            actual: format!("got {:?}", batch.column(3).data_type()),
            location: Some(CsvLocation {
                file: "parquet".into(),
                line: 0,
                column: Some("side".into()),
            }),
        })?;

    let n = batch.num_rows();
    let mut ticks = Vec::with_capacity(n);
    for i in 0..n {
        let side = match side_col.value(i).to_ascii_lowercase().as_str() {
            "buy" | "b" => Side::Buy,
            "sell" | "s" => Side::Sell,
            other => {
                return Err(DataError::CorruptData {
                    expected: "buy/sell".into(),
                    actual: format!("row {i}: '{other}'"),
                    location: Some(CsvLocation {
                        file: "parquet".into(),
                        line: i,
                        column: Some("side".into()),
                    }),
                })
            }
        };
        ticks.push(Tick::new(
            Timestamp::from_nanos(ts_col.value(i)),
            Price::from_f64(price_col.value(i)),
            Quantity::from_f64(qty_col.value(i)),
            side,
        ));
    }
    Ok(ticks)
}

#[async_trait]
impl DataSource for ParquetSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn schema(&self) -> &[SchemaField] {
        // 与 CsvSource 一致的稳定 leak 模式(此处泄漏的是 4 字段常量,可忽略不计)
        let s: Vec<SchemaField> = Self::schema_fields();
        Box::leak(s.into_boxed_slice()) as &[SchemaField]
    }

    async fn query(&self, req: &DataRequest) -> DataResult<Dataset> {
        // 阻塞 IO 卸载到独立线程池
        let path = self.path.clone();
        let name = self.name.clone();
        let req = req.clone();
        let batch_size = self.batch_size;

        let result = tokio::task::spawn_blocking(move || -> DataResult<Dataset> {
            let src = ParquetSource {
                name,
                path,
                batch_size,
            };
            src.load_sync(&req)
        })
        .await
        .map_err(|e| DataError::InvalidRequest(format!("join error: {e}")))??;

        Ok(result)
    }

    async fn stream(
        &self,
        req: &DataRequest,
    ) -> DataResult<Pin<Box<dyn Stream<Item = DataResult<Tick>> + Send>>> {
        let dataset = self.query(req).await?;
        let stream = futures::stream::iter(dataset.rows.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    // 单元测试覆盖由 tests/integration_test.rs 下的 parquet_fixtures 模块承担
    // 这里只放纯算法测试(无 IO)
    use super::*;
    use arrow::array::Int64Array;
    use std::sync::Arc;

    fn make_buy_int64_array(vals: Vec<i64>) -> Int64Array {
        Int64Array::from(vals)
    }

    #[test]
    fn batch_to_ticks_returns_empty_for_empty_batch() {
        // 构造 4 列空 batch
        let schema = Arc::new(arrow::datatypes::Schema::new(vec![
            arrow::datatypes::Field::new("ts", ArrowType::Int64, false),
            arrow::datatypes::Field::new("px", ArrowType::Float64, false),
            arrow::datatypes::Field::new("qty", ArrowType::Float64, false),
            arrow::datatypes::Field::new("side", ArrowType::Utf8, false),
        ]));
        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(make_buy_int64_array(vec![])),
                Arc::new(Float64Array::from(Vec::<f64>::new())),
                Arc::new(Float64Array::from(Vec::<f64>::new())),
                Arc::new(StringArray::from(Vec::<&str>::new())),
            ],
        )
        .unwrap();
        let ticks = batch_to_ticks(&batch).expect("empty batch ok");
        assert!(ticks.is_empty());
    }
}
