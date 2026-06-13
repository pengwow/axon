//! K 线数据集(PR6 新增)
//!
//! 内部用 Arrow `Vec<RecordBatch>` 列式存储，6 列 schema:
//! - timestamp: Int64 (纳秒)
//! - open: Float64
//! - high: Float64
//! - low: Float64
//! - close: Float64
//! - volume: Float64

use std::sync::{Arc, OnceLock};

use arrow::array::{Float64Array, Int64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use axon_core::market::Bar;
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};

use crate::error::{DataError, DataResult};
use crate::types::{DataRequest, Frequency};

/// 6 列 Bar schema(共享，所有 Bar 数据源复用)
pub fn bar_schema() -> &'static Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::Int64, false),
            Field::new("open", DataType::Float64, false),
            Field::new("high", DataType::Float64, false),
            Field::new("low", DataType::Float64, false),
            Field::new("close", DataType::Float64, false),
            Field::new("volume", DataType::Float64, false),
        ]))
    })
}

/// 共享工具:Bar 数组 → 按 batch_size 切分的 `Vec<RecordBatch>`
///
/// 空 bars 返回空 Vec(避免 Arrow schema 校验空 batch 报错)。
pub fn bars_to_batches(bars: &[Bar], batch_size: usize) -> DataResult<Vec<RecordBatch>> {
    if bars.is_empty() {
        return Ok(Vec::new());
    }
    let chunk_size = batch_size.max(1);
    let mut batches = Vec::new();
    for chunk in bars.chunks(chunk_size) {
        let ts_array: Int64Array = chunk.iter().map(|b| b.timestamp.nanos).collect();
        let open_array: Float64Array = chunk.iter().map(|b| b.open.as_f64()).collect();
        let high_array: Float64Array = chunk.iter().map(|b| b.high.as_f64()).collect();
        let low_array: Float64Array = chunk.iter().map(|b| b.low.as_f64()).collect();
        let close_array: Float64Array = chunk.iter().map(|b| b.close.as_f64()).collect();
        let volume_array: Float64Array = chunk.iter().map(|b| b.volume.as_f64()).collect();
        let batch = RecordBatch::try_new(
            bar_schema().clone(),
            vec![
                Arc::new(ts_array),
                Arc::new(open_array),
                Arc::new(high_array),
                Arc::new(low_array),
                Arc::new(close_array),
                Arc::new(volume_array),
            ],
        )
        .map_err(|e| DataError::Internal(format!("bars_to_batches try_new: {e}")))?;
        batches.push(batch);
    }
    Ok(batches)
}

/// K 线数据集(PR6 新增)
#[derive(Debug, Clone)]
pub struct BarDataset {
    /// 数据集 ID
    pub id: Uuid,
    /// Arrow schema(共享 bar_schema())
    pub schema: Arc<Schema>,
    /// 列式 batch 列表
    pub batches: Vec<RecordBatch>,
    /// 数据源名称
    pub source: String,
    /// 加载时间
    pub loaded_at: DateTime<Utc>,
    /// SHA256 校验和(沿用行式格式 "ts|o|h|l|c|v;")
    pub checksum: String,
    /// 关联请求(可追溯)
    pub request: DataRequest,
    /// Bar 的时间周期
    pub frequency: Frequency,
}

impl BarDataset {
    /// 构造新 Bar 数据集(从 `Vec<RecordBatch>` 直接构造)
    pub fn new(
        batches: Vec<RecordBatch>,
        source: String,
        request: DataRequest,
        frequency: Frequency,
    ) -> DataResult<Self> {
        let checksum = Self::compute_checksum(&batches);
        Ok(Self {
            id: Uuid::new_v4(),
            schema: bar_schema().clone(),
            batches,
            source,
            loaded_at: Utc::now(),
            checksum,
            request,
            frequency,
        })
    }

    /// 桥接入口:从 `Vec<Bar>` 一次性构造(测试用)
    pub fn from_bars(
        bars: Vec<Bar>,
        source: String,
        request: DataRequest,
        frequency: Frequency,
    ) -> DataResult<Self> {
        let batches = bars_to_batches(&bars, 1024)?;
        Self::new(batches, source, request, frequency)
    }

    /// 总行数(所有 batch.num_rows() 求和)
    pub fn len(&self) -> usize {
        self.batches.iter().map(|b| b.num_rows()).sum()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 行式迭代(零拷贝读 Arrow buffer → 构造 Bar)
    pub fn iter_rows(&self) -> impl Iterator<Item = Bar> + '_ {
        self.batches.iter().flat_map(|batch| {
            let ts = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("col 0 Int64Array");
            let open = batch
                .column(1)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 1 Float64Array");
            let high = batch
                .column(2)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 2 Float64Array");
            let low = batch
                .column(3)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 3 Float64Array");
            let close = batch
                .column(4)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 4 Float64Array");
            let volume = batch
                .column(5)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 5 Float64Array");
            (0..batch.num_rows()).map(move |i| Bar {
                timestamp: Timestamp::from_nanos(ts.value(i)),
                open: Price::from_f64(open.value(i)),
                high: Price::from_f64(high.value(i)),
                low: Price::from_f64(low.value(i)),
                close: Price::from_f64(close.value(i)),
                volume: Quantity::from_f64(volume.value(i)),
            })
        })
    }

    /// Batch 迭代
    pub fn iter_batches(&self) -> std::slice::Iter<'_, RecordBatch> {
        self.batches.iter()
    }

    /// 直接访问 batches 字段引用
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    /// 直接访问 schema 字段引用
    pub fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    /// 取出 batches 所有权
    pub fn into_batches(self) -> Vec<RecordBatch> {
        self.batches
    }

    /// 获取频率
    pub fn frequency(&self) -> Frequency {
        self.frequency
    }

    /// SHA256 校验和(沿用行式格式 "ts|o|h|l|c|v;")
    pub(crate) fn compute_checksum(batches: &[RecordBatch]) -> String {
        let mut hasher = Sha256::new();
        for batch in batches {
            let ts = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("col 0 Int64Array");
            let open = batch
                .column(1)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 1 Float64Array");
            let high = batch
                .column(2)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 2 Float64Array");
            let low = batch
                .column(3)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 3 Float64Array");
            let close = batch
                .column(4)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 4 Float64Array");
            let volume = batch
                .column(5)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 5 Float64Array");
            for i in 0..batch.num_rows() {
                let line = format!(
                    "{}|{}|{}|{}|{}|{};",
                    ts.value(i),
                    open.value(i),
                    high.value(i),
                    low.value(i),
                    close.value(i),
                    volume.value(i),
                );
                hasher.update(line.as_bytes());
            }
        }
        let digest = hasher.finalize();
        hex::encode(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bar(nanos: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Bar {
        Bar {
            timestamp: Timestamp::from_nanos(nanos),
            open: Price::from_f64(o),
            high: Price::from_f64(h),
            low: Price::from_f64(l),
            close: Price::from_f64(c),
            volume: Quantity::from_f64(v),
        }
    }

    fn make_bar_ds(bars: Vec<Bar>) -> BarDataset {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Min1);
        BarDataset::from_bars(bars, "test".into(), req, Frequency::Min1).expect("from_bars")
    }

    #[test]
    fn new_bar_dataset_has_uuid_and_checksum() {
        let ds = make_bar_ds(vec![make_bar(0, 100.0, 110.0, 90.0, 105.0, 1000.0)]);
        assert_eq!(ds.len(), 1);
        assert!(!ds.is_empty());
        assert_eq!(ds.checksum.len(), 64);
        assert_eq!(ds.frequency(), Frequency::Min1);
    }

    #[test]
    fn identical_bars_produce_identical_checksum() {
        let req_a = DataRequest::new("A", Utc::now(), Utc::now(), Frequency::Min1);
        let req_b = req_a.clone();
        let bars = vec![make_bar(0, 100.0, 110.0, 90.0, 105.0, 1000.0)];
        let ds1 = BarDataset::from_bars(bars.clone(), "a".into(), req_a, Frequency::Min1).unwrap();
        let ds2 = BarDataset::from_bars(bars, "b".into(), req_b, Frequency::Min1).unwrap();
        assert_eq!(ds1.checksum, ds2.checksum);
    }

    #[test]
    fn iter_rows_yields_all_bars() {
        let bars = (0..5)
            .map(|i| make_bar(i * 60_000_000_000, 100.0, 110.0, 90.0, 105.0, 100.0))
            .collect();
        let ds = make_bar_ds(bars);
        let rows: Vec<Bar> = ds.iter_rows().collect();
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn empty_bars_produce_empty_dataset() {
        let ds = make_bar_ds(vec![]);
        assert!(ds.is_empty());
        assert_eq!(ds.len(), 0);
    }

    #[test]
    fn batch_size_is_respected() {
        let bars: Vec<Bar> = (0..10)
            .map(|i| make_bar(i * 60_000_000_000, 100.0, 110.0, 90.0, 105.0, 100.0))
            .collect();
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Min1);
        let batches = bars_to_batches(&bars, 3).unwrap();
        assert_eq!(batches.len(), 4); // 10 / 3 = 3...1 → 4 batches
        let ds = BarDataset::new(batches, "test".into(), req, Frequency::Min1).unwrap();
        assert_eq!(ds.len(), 10);
    }
}
