//! 数据集
//!
//! 骨架阶段的内存表示:以 `Vec<Tick>` 存储,提供行式迭代与基础过滤。
//! 后续可扩展为 Arrow `RecordBatch` 实现(见 design `04-data-service.md` M2)。

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use axon_core::market::Tick;
use axon_core::time::Timestamp;

use crate::types::{DataRequest, SchemaField};

/// 数据集(零拷贝目标:用 `Vec<Tick>` 占位,后续替换为 Arrow)
#[derive(Debug, Clone)]
pub struct Dataset {
    /// 数据集 ID
    pub id: Uuid,
    /// 内部行存储
    pub rows: Vec<Tick>,
    /// 字段 schema
    pub schema: Vec<SchemaField>,
    /// 数据源名称
    pub source: String,
    /// 加载时间
    pub loaded_at: DateTime<Utc>,
    /// SHA256 校验和(`Vec<Tick>` 字节表示)
    pub checksum: String,
    /// 关联请求(可追溯)
    pub request: DataRequest,
}

impl Dataset {
    /// 构造新数据集(自动计算 checksum + UUID)
    ///
    /// # Examples
    ///
    /// ```
    /// use axon_data::{Dataset, DataRequest, Frequency};
    /// use axon_core::market::{Side, Tick};
    /// use axon_core::time::Timestamp;
    /// use axon_core::types::{Price, Quantity};
    /// use chrono::Utc;
    ///
    /// let tick = Tick::new(
    ///     Timestamp::from_nanos(0),
    ///     Price::from_f64(100.0),
    ///     Quantity::from(1.0),
    ///     Side::Buy,
    /// );
    /// let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
    /// let ds = Dataset::new(vec![tick], vec![], "mock".into(), req);
    /// assert_eq!(ds.len(), 1);
    /// assert!(!ds.checksum.is_empty());
    /// ```
    pub fn new(rows: Vec<Tick>, schema: Vec<SchemaField>, source: String, request: DataRequest) -> Self {
        let checksum = Self::compute_checksum(&rows);
        Self {
            id: Uuid::new_v4(),
            rows,
            schema,
            source,
            loaded_at: Utc::now(),
            checksum,
            request,
        }
    }

    /// 行数
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 行式迭代
    pub fn iter(&self) -> std::slice::Iter<'_, Tick> {
        self.rows.iter()
    }

    /// 按谓词过滤(返回新 Dataset,保留元数据)
    pub fn filter<F: Fn(&Tick) -> bool>(&self, f: F) -> Dataset {
        let rows: Vec<Tick> = self.rows.iter().copied().filter(|t| f(t)).collect();
        Dataset::new(rows, self.schema.clone(), self.source.clone(), self.request.clone())
    }

    /// 取前 n 行(若 n > len 则返回全部)
    pub fn take(&self, n: usize) -> Dataset {
        let n = n.min(self.rows.len());
        let rows = self.rows[..n].to_vec();
        Dataset::new(rows, self.schema.clone(), self.source.clone(), self.request.clone())
    }

    /// 跳过前 n 行
    pub fn skip(&self, n: usize) -> Dataset {
        let n = n.min(self.rows.len());
        let rows = self.rows[n..].to_vec();
        Dataset::new(rows, self.schema.clone(), self.source.clone(), self.request.clone())
    }

    /// 取最后 n 行
    pub fn last_n(&self, n: usize) -> Dataset {
        let n = n.min(self.rows.len());
        let start = self.rows.len() - n;
        let rows = self.rows[start..].to_vec();
        Dataset::new(rows, self.schema.clone(), self.source.clone(), self.request.clone())
    }

    /// 按时间窗口过滤(包含两端)
    pub fn by_time_range(&self, start: Timestamp, end: Timestamp) -> Dataset {
        let rows: Vec<Tick> = self
            .rows
            .iter()
            .copied()
            .filter(|t| {
                let ts = t.timestamp.nanos;
                ts >= start.nanos && ts <= end.nanos
            })
            .collect();
        Dataset::new(rows, self.schema.clone(), self.source.clone(), self.request.clone())
    }

    /// 校验和计算(SHA256 拼 Tick 字段的字符串表示,safe 实现)
    pub(crate) fn compute_checksum(rows: &[Tick]) -> String {
        let mut hasher = Sha256::new();
        for tick in rows {
            // 用 safe 字符串拼接(虽然不是严格的 zero-copy,但避免 unsafe 块)
            // Tick 字段都是 `Copy` 的,序列化开销可忽略
            let line = format!(
                "{}|{}|{}|{:?};",
                tick.timestamp.nanos,
                tick.price.as_f64(),
                tick.quantity.as_f64(),
                tick.side,
            );
            hasher.update(line.as_bytes());
        }
        let digest = hasher.finalize();
        hex::encode(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Frequency;
    use axon_core::market::Side;
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

    #[test]
    fn new_dataset_has_uuid_and_checksum() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = Dataset::new(vec![make_tick(1), make_tick(2)], vec![], "test".into(), req);
        assert_eq!(ds.len(), 2);
        assert!(!ds.is_empty());
        assert_eq!(ds.checksum.len(), 64); // SHA256 hex
    }

    #[test]
    fn identical_rows_produce_identical_checksum() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let ds1 = Dataset::new(vec![make_tick(1), make_tick(2)], vec![], "a".into(), req.clone());
        let ds2 = Dataset::new(vec![make_tick(1), make_tick(2)], vec![], "b".into(), req);
        // checksum 只依赖 rows,不应受 source/loaded_at 影响
        assert_eq!(ds1.checksum, ds2.checksum);
    }

    #[test]
    fn iter_yields_all_rows() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let rows: Vec<Tick> = (0..5).map(make_tick).collect();
        let ds = Dataset::new(rows.clone(), vec![], "test".into(), req);
        assert_eq!(ds.iter().count(), 5);
    }

    #[test]
    fn filter_keeps_only_matching_ticks() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let rows: Vec<Tick> = (0..4).map(make_tick).collect(); // prices: 100, 101, 102, 103
        let ds = Dataset::new(rows, vec![], "test".into(), req);
        let filtered = ds.filter(|t| t.price.as_f64() > 101.5);
        assert_eq!(filtered.len(), 2);
        let prices: Vec<f64> = filtered.iter().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![102.0, 103.0]);
    }

    #[test]
    fn take_returns_first_n_rows() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let rows: Vec<Tick> = (0..5).map(make_tick).collect(); // prices: 100..104
        let ds = Dataset::new(rows, vec![], "test".into(), req);
        let t = ds.take(3);
        assert_eq!(t.len(), 3);
        let prices: Vec<f64> = t.iter().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![100.0, 101.0, 102.0]);
    }

    #[test]
    fn skip_drops_first_n_rows() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let rows: Vec<Tick> = (0..5).map(make_tick).collect();
        let ds = Dataset::new(rows, vec![], "test".into(), req);
        let s = ds.skip(2);
        assert_eq!(s.len(), 3);
        let prices: Vec<f64> = s.iter().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![102.0, 103.0, 104.0]);
    }

    #[test]
    fn last_n_returns_final_n_rows() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let rows: Vec<Tick> = (0..5).map(make_tick).collect();
        let ds = Dataset::new(rows, vec![], "test".into(), req);
        let l = ds.last_n(2);
        let prices: Vec<f64> = l.iter().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![103.0, 104.0]);
    }

    #[test]
    fn take_n_larger_than_len_returns_all() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let rows: Vec<Tick> = (0..2).map(make_tick).collect();
        let ds = Dataset::new(rows, vec![], "test".into(), req);
        assert_eq!(ds.take(10).len(), 2);
    }

    #[test]
    fn by_time_range_keeps_only_in_window() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        // 创建 5 个时间戳:0, 1e9, 2e9, 3e9, 4e9 纳秒
        let rows: Vec<Tick> = (0..5)
            .map(|i| {
                Tick::new(
                    Timestamp::from_nanos(i as i64 * 1_000_000_000),
                    Price::from_f64(100.0 + i as f64),
                    Quantity::from(1.0),
                    Side::Buy,
                )
            })
            .collect();
        let ds = Dataset::new(rows, vec![], "test".into(), req);
        let r = ds.by_time_range(Timestamp::from_nanos(1_000_000_000), Timestamp::from_nanos(3_000_000_000));
        let ts: Vec<i64> = r.iter().map(|t| t.timestamp.nanos).collect();
        assert_eq!(ts, vec![1_000_000_000, 2_000_000_000, 3_000_000_000]);
    }

    #[test]
    fn by_time_range_empty_when_no_match() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let rows: Vec<Tick> = (0..3).map(make_tick).collect();
        let ds = Dataset::new(rows, vec![], "test".into(), req);
        let r = ds.by_time_range(Timestamp::from_nanos(500_000_000_000), Timestamp::from_nanos(1_000_000_000_000));
        assert!(r.is_empty());
    }
}
