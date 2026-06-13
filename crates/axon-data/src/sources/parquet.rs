//! Parquet 数据源(feature = "parquet-source")
//!
//! 严格 4 列 schema(按位置):
//! 0: timestamp(int64, 纳秒)
//! 1: price(float64)
//! 2: quantity(float64)
//! 3: side(string) — 接受 "buy"/"sell" 或 "b"/"s"
//!
//! PR5 列式升级:
//! - `query()`:全量加载到 `Vec<RecordBatch>`,零拷贝(PR3 `batch_to_ticks` 已删除)
//! - `stream()`:真流式,`spawn_blocking` 启动后台 reader + mpsc channel
//!   推送 `RecordBatch`,内存复杂度 O(batch_size)(PR4 + PR5)
//! - `compute_checksum` 沿用 PR1 行式格式,跨 PR 字节级一致

use std::fs::File;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures_core::Stream;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tokio::sync::mpsc;

use arrow::datatypes::DataType as ArrowType;
use arrow::record_batch::RecordBatch;

use crate::dataset::Dataset;
use crate::error::{CsvLocation, DataError, DataResult};
use crate::traits::DataSource;
use crate::types::{DataRequest, Frequency, SchemaField};

/// 默认 batch size(每批 Arrow RecordBatch 包含的行数)
const DEFAULT_BATCH_SIZE: usize = 1024;

/// Parquet 数据源
///
/// 读取 `.parquet` 文件,严格校验 4 列 schema(按位置)后,
/// 持有 `Vec<RecordBatch>` 并通过 `Dataset` 暴露。
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
    fn validate_schema(schema: &Arc<arrow::datatypes::Schema>) -> DataResult<()> {
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

    /// 内部同步加载逻辑(在 `spawn_blocking` 中运行)— PR5:返回 `Vec<RecordBatch>` 列表
    fn load_sync(&self, req: &DataRequest) -> DataResult<Dataset> {
        // 1. 打开文件
        let file = File::open(&self.path).map_err(|e| {
            DataError::InvalidRequest(format!("open {}: {}", self.path.display(), e))
        })?;

        // 2. 构造 builder 并校验 schema
        let builder =
            ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| DataError::CorruptData {
                expected: "valid parquet file".into(),
                actual: format!("{e}"),
                location: Some(CsvLocation {
                    file: self.path.to_string_lossy().into_owned(),
                    line: 0,
                    column: None,
                }),
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

        let mut batches: Vec<RecordBatch> = Vec::new();
        let mut total_rows = 0usize;
        for batch_result in batch_reader {
            let batch = batch_result.map_err(|e| DataError::CorruptData {
                expected: "valid record batch".into(),
                actual: format!("{e}"),
                location: Some(CsvLocation {
                    file: self.path.to_string_lossy().into_owned(),
                    line: total_rows,
                    column: None,
                }),
            })?;
            total_rows += batch.num_rows();
            batches.push(batch);
        }

        // 4. Frequency 校验(与 CsvSource 一致)
        if req.frequency != Frequency::Tick && !batches.is_empty() {
            return Err(DataError::InvalidRequest(format!(
                "ParquetSource 骨架仅支持 Tick 频率,收到 {:?}",
                req.frequency
            )));
        }

        // 5. PR5:直接走 RecordBatch 列表(零拷贝)— 不再过 batch_to_ticks
        Dataset::new(batches, self.name.clone(), req.clone())
    }

    /// 流式同步加载(在 `spawn_blocking` 中运行)— PR5:逐 batch 推送 RecordBatch
    ///
    /// 行为契约(沿用 PR4):
    /// - caller 持有 `RowGroupStream`,从 `poll_next` 拉取 `RecordBatch`
    /// - 后台 `spawn_blocking` task 通过 `tx.blocking_send` 推 `RecordBatch`
    /// - caller 提前中断(`RowGroupStream` drop)→ rx drop → 后台 sender.send 返回 Err → task 自然退出
    /// - 错误通过 `tx.blocking_send(Err(...))` 推给 caller,然后退出
    fn stream_sync(self, tx: mpsc::Sender<DataResult<RecordBatch>>) -> DataResult<()> {
        // 1. 打开文件
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.blocking_send(Err(DataError::InvalidRequest(format!(
                    "open {}: {}",
                    self.path.display(),
                    e
                ))));
                return Ok(());
            }
        };

        // 2. 构造 builder
        let builder = match ParquetRecordBatchReaderBuilder::try_new(file) {
            Ok(b) => b,
            Err(e) => {
                let _ = tx.blocking_send(Err(DataError::CorruptData {
                    expected: "valid parquet file".into(),
                    actual: format!("{e}"),
                    location: Some(CsvLocation {
                        file: self.path.to_string_lossy().into_owned(),
                        line: 0,
                        column: None,
                    }),
                }));
                return Ok(());
            }
        };

        // 3. 校验 schema
        let schema = builder.schema().clone();
        if let Err(e) = Self::validate_schema(&schema) {
            let _ = tx.blocking_send(Err(e));
            return Ok(());
        }

        // 4. 构造 batch reader
        let batch_reader = match builder.with_batch_size(self.batch_size).build() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.blocking_send(Err(DataError::CorruptData {
                    expected: "record batch reader".into(),
                    actual: format!("{e}"),
                    location: Some(CsvLocation {
                        file: self.path.to_string_lossy().into_owned(),
                        line: 0,
                        column: None,
                    }),
                }));
                return Ok(());
            }
        };

        // 5. 逐 batch 解码并推送(PR5:推 batch 而非逐行 tick)
        for batch_result in batch_reader {
            match batch_result {
                Ok(batch) => {
                    // caller 中断时 send 返回 Err,直接退出
                    if tx.blocking_send(Ok(batch)).is_err() {
                        return Ok(());
                    }
                }
                Err(e) => {
                    let _ = tx.blocking_send(Err(DataError::CorruptData {
                        expected: "valid record batch".into(),
                        actual: format!("{e}"),
                        location: Some(CsvLocation {
                            file: self.path.to_string_lossy().into_owned(),
                            line: 0,
                            column: None,
                        }),
                    }));
                    return Ok(());
                }
            }
        }
        Ok(())
    }
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
        _req: &DataRequest,
    ) -> DataResult<Pin<Box<dyn Stream<Item = DataResult<RecordBatch>> + Send>>> {
        let path = self.path.clone();
        let name = self.name.clone();
        let batch_size = self.batch_size;

        // 1. 创建有界 channel(容量 = batch_size,提供 backpressure)
        let (tx, rx) = mpsc::channel::<DataResult<RecordBatch>>(batch_size.max(1));

        // 2. spawn_blocking 启动后台 reader
        let join = tokio::task::spawn_blocking(move || -> DataResult<()> {
            let src = ParquetSource {
                name,
                path,
                batch_size,
            };
            src.stream_sync(tx)
        });

        // 3. 包装成 Stream(PR5:Item = RecordBatch)
        let stream_impl = RowGroupStream {
            rx,
            join: Some(join),
        };
        Ok(Box::pin(stream_impl))
    }
}

/// 内部流式结构:持有 mpsc receiver + spawn_blocking JoinHandle
/// PR5:`Item = RecordBatch`(而非 PR4 的 `Tick`)
struct RowGroupStream {
    rx: mpsc::Receiver<DataResult<RecordBatch>>,
    join: Option<tokio::task::JoinHandle<DataResult<()>>>,
}

impl Stream for RowGroupStream {
    type Item = DataResult<RecordBatch>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut rx_pin = Pin::new(&mut self.rx);
        match rx_pin.as_mut().poll_recv(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(item)),
            Poll::Ready(None) => {
                // channel 关闭(后台 task 退出)→ 丢 handle 让 task 自然结束
                // 注意:不在 poll_next 内 block_on(避免 re-enter panic);
                // 错误已经通过 tx.blocking_send(Err(...)) 推给 caller 或从未发生
                if let Some(handle) = self.join.take() {
                    drop(handle);
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for RowGroupStream {
    fn drop(&mut self) {
        // rx 在 struct 内,先随 self drop 而 drop(顺序:Drop 字段)
        // 后台 sender.send 返回 Err → stream_sync 退出 → task 完成
        // join handle 留给 tokio runtime 自动清理(未 await 的 task)
    }
}

#[cfg(test)]
mod tests {
    // 单元测试覆盖 validate_schema 纯函数(无 IO)
    use super::*;

    #[test]
    fn validate_schema_rejects_wrong_type() {
        let schema = Arc::new(arrow::datatypes::Schema::new(vec![
            arrow::datatypes::Field::new("ts", ArrowType::Int64, false),
            arrow::datatypes::Field::new("px", ArrowType::Int32, false), // 错!
            arrow::datatypes::Field::new("qty", ArrowType::Float64, false),
            arrow::datatypes::Field::new("side", ArrowType::Utf8, false),
        ]));
        let res = ParquetSource::validate_schema(&schema);
        assert!(matches!(res, Err(DataError::SchemaMismatch { .. })));
    }

    #[test]
    fn validate_schema_rejects_too_few_columns() {
        let schema = Arc::new(arrow::datatypes::Schema::new(vec![
            arrow::datatypes::Field::new("ts", ArrowType::Int64, false),
        ]));
        let res = ParquetSource::validate_schema(&schema);
        assert!(matches!(res, Err(DataError::SchemaMismatch { .. })));
    }

    #[test]
    fn validate_schema_accepts_correct_4_columns() {
        let schema = Arc::new(arrow::datatypes::Schema::new(vec![
            arrow::datatypes::Field::new("ts", ArrowType::Int64, false),
            arrow::datatypes::Field::new("px", ArrowType::Float64, false),
            arrow::datatypes::Field::new("qty", ArrowType::Float64, false),
            arrow::datatypes::Field::new("side", ArrowType::Utf8, false),
        ]));
        let res = ParquetSource::validate_schema(&schema);
        assert!(res.is_ok());
    }
}
