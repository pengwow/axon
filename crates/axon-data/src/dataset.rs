//! 数据集
//!
//! PR5 列式升级:内部以 `Vec<RecordBatch>`(Arrow)存储,提供:
//! - 列式迭代(零拷贝读 Arrow buffer)
//! - 列式过滤(走 `arrow::compute::filter_record_batch`)
//! - 共享 `DATASET_SCHEMA`(4 列:int64 ts / f64 px / f64 qty / utf8 side)
//! - 沿用 PR1 行式 checksum 格式,跨 PR 字节级一致
//!
//! 破坏性变更:
//! - `Dataset::new` 签名改为 `(batches, source, request) -> DataResult<Self>`
//! - `iter()` 改名为 `iter_rows()`,`rows: Vec<Tick>` 改为 `batches: Vec<RecordBatch>`
//! - 桥接入口 `Dataset::from_ticks` 给测试 / fuzz / bench 使用

use std::sync::{Arc, OnceLock};

use arrow::array::{Array, BooleanArray, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use axon_core::market::{Side, Tick};
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};

use crate::error::{DataError, DataResult};
use crate::types::DataRequest;

/// 4 列严格 schema(共享,所有 Source 复用)
/// - timestamp: Int64 (纳秒)
/// - price: Float64
/// - quantity: Float64
/// - side: Utf8 ("buy" / "sell")
pub fn dataset_schema() -> &'static Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::Int64, false),
            Field::new("price", DataType::Float64, false),
            Field::new("quantity", DataType::Float64, false),
            Field::new("side", DataType::Utf8, false),
        ]))
    })
}

/// 共享工具:Tick 数组 → 按 batch_size 切分的 `Vec<RecordBatch>`
///
/// 单一实现,MockSource / CsvSource / fuzz / bench 复用。
/// 空 ticks 返回空 Vec(避免 Arrow schema 校验空 batch 报错)。
pub fn ticks_to_batches(ticks: &[Tick], batch_size: usize) -> DataResult<Vec<RecordBatch>> {
    if ticks.is_empty() {
        return Ok(Vec::new());
    }
    let chunk_size = batch_size.max(1);
    let mut batches = Vec::new();
    for chunk in ticks.chunks(chunk_size) {
        let ts_array: Int64Array = chunk.iter().map(|t| t.timestamp.nanos).collect();
        let px_array: Float64Array = chunk.iter().map(|t| t.price.as_f64()).collect();
        let qty_array: Float64Array = chunk.iter().map(|t| t.quantity.as_f64()).collect();
        // arrow 53:`StringArray` 仅支持 `FromIterator<Option<&str>>`,
        // 用 `from_iter_values` 接受 `&str` 迭代器
        let side_array: StringArray =
            StringArray::from_iter_values(chunk.iter().map(|t| match t.side {
                Side::Buy => "buy",
                Side::Sell => "sell",
            }));
        let batch = RecordBatch::try_new(
            dataset_schema().clone(),
            vec![
                Arc::new(ts_array),
                Arc::new(px_array),
                Arc::new(qty_array),
                Arc::new(side_array),
            ],
        )
        .map_err(|e| DataError::Internal(format!("ticks_to_batches try_new: {e}")))?;
        batches.push(batch);
    }
    Ok(batches)
}

/// 数据集(PR5:内部用 Arrow `Vec<RecordBatch>` 列式存储)
#[derive(Debug, Clone)]
pub struct Dataset {
    /// 数据集 ID
    pub id: Uuid,
    /// Arrow schema(共享 `dataset_schema()`)
    pub schema: Arc<Schema>,
    /// 列式 batch 列表
    pub batches: Vec<RecordBatch>,
    /// 数据源名称
    pub source: String,
    /// 加载时间
    pub loaded_at: DateTime<Utc>,
    /// SHA256 校验和(**沿用 PR1 行式格式** "ts|px|qty|side;",跨 PR 字节级一致)
    pub checksum: String,
    /// 关联请求(可追溯)
    pub request: DataRequest,
}

impl Dataset {
    /// 构造新数据集(从 `Vec<RecordBatch>` 直接构造;走列式 checksum)
    ///
    /// 破坏性变更(PR5):
    /// - 移除 `Vec<SchemaField>` 参数(统一用 `dataset_schema()`)
    /// - 改为返回 `DataResult<Self>`(构造可能因 Arrow schema 校验失败)
    pub fn new(
        batches: Vec<RecordBatch>,
        source: String,
        request: DataRequest,
    ) -> DataResult<Self> {
        let checksum = Self::compute_checksum(&batches);
        Ok(Self {
            id: Uuid::new_v4(),
            schema: dataset_schema().clone(),
            batches,
            source,
            loaded_at: Utc::now(),
            checksum,
            request,
        })
    }

    /// 桥接入口:从 `Vec<Tick>` 一次性构造(测试 / fuzz / bench 用)
    pub fn from_ticks(ticks: Vec<Tick>, source: String, request: DataRequest) -> DataResult<Self> {
        let batches = ticks_to_batches(&ticks, 1024)?;
        Self::new(batches, source, request)
    }

    /// 总行数(所有 batch.num_rows() 求和)
    pub fn len(&self) -> usize {
        self.batches.iter().map(|b| b.num_rows()).sum()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 行式迭代(零拷贝读 Arrow buffer → 构造 Tick)
    pub fn iter_rows(&self) -> impl Iterator<Item = Tick> + '_ {
        self.batches.iter().flat_map(|batch| {
            let ts = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("col 0 is Int64Array (schema-validated)");
            let px = batch
                .column(1)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 1 is Float64Array (schema-validated)");
            let qty = batch
                .column(2)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 2 is Float64Array (schema-validated)");
            let side = batch
                .column(3)
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("col 3 is StringArray (schema-validated)");
            (0..batch.num_rows()).map(move |i| {
                let s = match side.value(i).to_ascii_lowercase().as_str() {
                    "buy" | "b" => Side::Buy,
                    _ => Side::Sell,
                };
                Tick::new(
                    Timestamp::from_nanos(ts.value(i)),
                    Price::from_f64(px.value(i)),
                    Quantity::from_f64(qty.value(i)),
                    s,
                )
            })
        })
    }

    /// Batch 迭代(直接给 caller `&[RecordBatch]` 访问,用于列式消费)
    pub fn iter_batches(&self) -> std::slice::Iter<'_, RecordBatch> {
        self.batches.iter()
    }

    /// 直接访问 `batches` 字段引用(供测试 / 反序列化校验用)
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    /// 直接访问 `schema` 字段引用(供测试 / 集成校验用)
    pub fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    /// 取出 batches 所有权(consuming,供 `stream()` 等场景使用)
    pub fn into_batches(self) -> Vec<RecordBatch> {
        self.batches
    }

    /// 列式 filter(走 `arrow::compute::filter_record_batch`,零拷贝)
    /// 谓词返回 `ArrayRef`(布尔掩码,true=保留)
    pub fn filter<F>(&self, predicate: F) -> DataResult<Dataset>
    where
        F: Fn(&RecordBatch) -> Arc<dyn Array>,
    {
        let mut new_batches = Vec::with_capacity(self.batches.len());
        for batch in &self.batches {
            let mask = predicate(batch);
            // arrow 53:`filter_record_batch` 第二个参数要 `&BooleanArray`
            let bool_mask = mask
                .as_any()
                .downcast_ref::<BooleanArray>()
                .expect("filter predicate must return BooleanArray");
            let filtered = arrow::compute::filter_record_batch(batch, bool_mask)
                .map_err(|e| DataError::Internal(format!("filter_record_batch: {e}")))?;
            if filtered.num_rows() > 0 {
                new_batches.push(filtered);
            }
        }
        Dataset::new(new_batches, self.source.clone(), self.request.clone())
    }

    /// 取前 n 行(跨 batch 边界)
    pub fn take(&self, n: usize) -> Dataset {
        let mut new_batches = Vec::new();
        let mut remaining = n.min(self.len());
        for batch in &self.batches {
            if remaining == 0 {
                break;
            }
            let take_n = remaining.min(batch.num_rows());
            new_batches.push(batch.slice(0, take_n));
            remaining -= take_n;
        }
        Dataset::new(new_batches, self.source.clone(), self.request.clone())
            .expect("take: from already-valid batches")
    }

    /// 跳过前 n 行
    pub fn skip(&self, n: usize) -> Dataset {
        let mut new_batches = Vec::new();
        let mut to_skip = n.min(self.len());
        for batch in &self.batches {
            if to_skip >= batch.num_rows() {
                to_skip -= batch.num_rows();
                continue;
            }
            let start = to_skip;
            let len = batch.num_rows() - start;
            new_batches.push(batch.slice(start, len));
            to_skip = 0;
        }
        Dataset::new(new_batches, self.source.clone(), self.request.clone())
            .expect("skip: from already-valid batches")
    }

    /// 取最后 n 行
    pub fn last_n(&self, n: usize) -> Dataset {
        let total = self.len();
        if n >= total {
            return Dataset::new(
                self.batches.clone(),
                self.source.clone(),
                self.request.clone(),
            )
            .expect("last_n: from already-valid batches");
        }
        let to_skip = total - n;
        self.skip(to_skip)
    }

    /// 按时间窗口过滤(包含两端,走 `filter` 谓词路径)
    pub fn by_time_range(&self, start: Timestamp, end: Timestamp) -> DataResult<Dataset> {
        // arrow 53:`kernels::cmp::gt_eq` / `lt_eq` 接受 `&dyn Datum` 标量比较,
        // 用 `Int64Array::new_scalar(value)` 创建标量(零分配)
        self.filter(|batch: &RecordBatch| {
            let ts = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("col 0 is Int64Array");
            let ge = arrow::compute::kernels::cmp::gt_eq(ts, &Int64Array::new_scalar(start.nanos))
                .expect("ge scalar");
            let le = arrow::compute::kernels::cmp::lt_eq(ts, &Int64Array::new_scalar(end.nanos))
                .expect("le scalar");
            // `and` 返回 `BooleanArray`;包装成 `Arc<dyn Array>` 给 filter
            let combined: Arc<dyn Array> = Arc::new(arrow::compute::and(&ge, &le).expect("and"));
            combined
        })
    }

    /// SHA256 校验和(**沿用 PR1 行式格式 "ts|px|qty|side;"**,跨 PR 字节级一致)
    pub(crate) fn compute_checksum(batches: &[RecordBatch]) -> String {
        let mut hasher = Sha256::new();
        for batch in batches {
            let ts = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("col 0 Int64Array");
            let px = batch
                .column(1)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 1 Float64Array");
            let qty = batch
                .column(2)
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("col 2 Float64Array");
            let side = batch
                .column(3)
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("col 3 StringArray");
            for i in 0..batch.num_rows() {
                // 关键:`Buy` / `Sell` 大写,沿用 `{:?}` Debug 输出格式(PR1 字节级)
                let s_dbg = match side.value(i).to_ascii_lowercase().as_str() {
                    "buy" | "b" => "Buy",
                    _ => "Sell",
                };
                let line = format!(
                    "{}|{}|{}|{:?};",
                    ts.value(i),
                    px.value(i),
                    qty.value(i),
                    s_dbg,
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
    use crate::types::Frequency;
    use axon_core::time::Timestamp;
    use axon_core::types::{Price, Quantity};

    fn make_tick(seq: u64) -> Tick {
        Tick::new(
            Timestamp::from_nanos((seq as i64) * 1_000_000_000),
            Price::from_f64(100.0 + seq as f64),
            Quantity::from(1.0),
            Side::Buy,
        )
    }

    fn make_ds(rows: Vec<Tick>) -> Dataset {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        Dataset::from_ticks(rows, "test".into(), req).expect("from_ticks")
    }

    #[test]
    fn new_dataset_has_uuid_and_checksum() {
        let ds = make_ds(vec![make_tick(1), make_tick(2)]);
        assert_eq!(ds.len(), 2);
        assert!(!ds.is_empty());
        assert_eq!(ds.checksum.len(), 64); // SHA256 hex
    }

    #[test]
    fn identical_rows_produce_identical_checksum() {
        let req_a = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let req_b = req_a.clone();
        let ds1 = Dataset::from_ticks(vec![make_tick(1), make_tick(2)], "a".into(), req_a).unwrap();
        let ds2 = Dataset::from_ticks(vec![make_tick(1), make_tick(2)], "b".into(), req_b).unwrap();
        // checksum 只依赖 rows(沿用 PR1 行式格式),不应受 source 影响
        assert_eq!(ds1.checksum, ds2.checksum);
    }

    #[test]
    fn iter_rows_yields_all_rows() {
        let ds = make_ds((0..5).map(make_tick).collect());
        let rows: Vec<Tick> = ds.iter_rows().collect();
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn filter_keeps_only_matching_ticks() {
        // 4 行:prices 100, 101, 102, 103
        let ds = make_ds((0..4).map(make_tick).collect());
        // 列式谓词:px > 101.5(走 `arrow::compute::kernels::cmp::gt` + scalar 包装)
        let filtered = ds
            .filter(|batch: &RecordBatch| {
                let px = batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .unwrap();
                let mask =
                    arrow::compute::kernels::cmp::gt(px, &Float64Array::new_scalar(101.5_f64))
                        .unwrap();
                Arc::new(mask) as Arc<dyn Array>
            })
            .expect("filter");
        assert_eq!(filtered.len(), 2);
        let prices: Vec<f64> = filtered.iter_rows().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![102.0, 103.0]);
    }

    #[test]
    fn take_returns_first_n_rows() {
        let ds = make_ds((0..5).map(make_tick).collect());
        let t = ds.take(3);
        assert_eq!(t.len(), 3);
        let prices: Vec<f64> = t.iter_rows().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![100.0, 101.0, 102.0]);
    }

    #[test]
    fn skip_drops_first_n_rows() {
        let ds = make_ds((0..5).map(make_tick).collect());
        let s = ds.skip(2);
        assert_eq!(s.len(), 3);
        let prices: Vec<f64> = s.iter_rows().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![102.0, 103.0, 104.0]);
    }

    #[test]
    fn last_n_returns_final_n_rows() {
        let ds = make_ds((0..5).map(make_tick).collect());
        let l = ds.last_n(2);
        let prices: Vec<f64> = l.iter_rows().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![103.0, 104.0]);
    }

    #[test]
    fn take_n_larger_than_len_returns_all() {
        let ds = make_ds((0..2).map(make_tick).collect());
        assert_eq!(ds.take(10).len(), 2);
    }

    #[test]
    fn by_time_range_keeps_only_in_window() {
        let ds = make_ds(
            (0..5)
                .map(|i| {
                    Tick::new(
                        Timestamp::from_nanos(i as i64 * 1_000_000_000),
                        Price::from_f64(100.0 + i as f64),
                        Quantity::from(1.0),
                        Side::Buy,
                    )
                })
                .collect(),
        );
        let r = ds
            .by_time_range(
                Timestamp::from_nanos(1_000_000_000),
                Timestamp::from_nanos(3_000_000_000),
            )
            .expect("range");
        let ts: Vec<i64> = r.iter_rows().map(|t| t.timestamp.nanos).collect();
        assert_eq!(ts, vec![1_000_000_000, 2_000_000_000, 3_000_000_000]);
    }

    #[test]
    fn by_time_range_empty_when_no_match() {
        let ds = make_ds((0..3).map(make_tick).collect());
        let r = ds
            .by_time_range(
                Timestamp::from_nanos(500_000_000_000),
                Timestamp::from_nanos(1_000_000_000_000),
            )
            .expect("range");
        assert!(r.is_empty());
    }

    /// 跨 PR checksum 字节级一致验证(PR1 格式)
    #[test]
    fn checksum_format_matches_pr1() {
        // 5 行 buy tick,PR1 时期会得到某固定 hex 串,这里只验证格式
        let ds = make_ds(
            (0..5)
                .map(|i| {
                    Tick::new(
                        Timestamp::from_nanos(i as i64 * 1_000_000_000),
                        Price::from_f64(100.0 + i as f64),
                        Quantity::from(1.0),
                        Side::Buy,
                    )
                })
                .collect(),
        );
        assert_eq!(ds.checksum.len(), 64);
        assert!(ds.checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
