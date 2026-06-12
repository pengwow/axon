//! 数据集
//!
//! 骨架阶段的内存表示:以 `Vec<Tick>` 存储,提供行式迭代与基础过滤。
//! 后续可扩展为 Arrow `RecordBatch` 实现(见 design `04-data-service.md` M2)。

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use axon_core::market::Tick;

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

    /// 校验和计算(SHA256 拼 Tick 字段的字符串表示,safe 实现)
    fn compute_checksum(rows: &[Tick]) -> String {
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
}
