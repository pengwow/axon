//! CSV 数据源(需 `csv-source` feature)
//!
//! 支持灵活列映射 + 时间戳单位转换 + 时间窗口过滤。
//! 默认假设列序:`timestamp,price,quantity,side`(纳秒整数时间戳)。

use async_trait::async_trait;
use futures_core::Stream;
use std::pin::Pin;

use crate::dataset::Dataset;
use crate::error::{CsvLocation, DataError, DataResult};
use crate::traits::DataSource;
use crate::types::{DataRequest, Frequency, SchemaField};

use axon_core::market::{Side, Tick};
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};

/// 时间戳单位(支持纳秒/微秒/毫秒/秒)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampUnit {
    /// 纳秒(默认)
    Nanos,
    /// 微秒
    Micros,
    /// 毫秒
    Millis,
    /// 秒
    Secs,
}

impl TimestampUnit {
    /// 转为纳秒(乘以对应系数)
    pub fn to_nanos(self, raw: i64) -> i64 {
        match self {
            TimestampUnit::Nanos => raw,
            TimestampUnit::Micros => raw.saturating_mul(1_000),
            TimestampUnit::Millis => raw.saturating_mul(1_000_000),
            TimestampUnit::Secs => raw.saturating_mul(1_000_000_000),
        }
    }
}

/// CSV 列映射(灵活指定各字段对应列号)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsvColumnMapping {
    /// 时间戳列(0-indexed)
    pub timestamp_col: usize,
    /// 价列
    pub price_col: usize,
    /// 量列
    pub quantity_col: usize,
    /// 买卖方向列(可选)
    pub side_col: Option<usize>,
    /// 时间戳单位
    pub timestamp_unit: TimestampUnit,
}

impl Default for CsvColumnMapping {
    fn default() -> Self {
        Self {
            timestamp_col: 0,
            price_col: 1,
            quantity_col: 2,
            side_col: Some(3),
            timestamp_unit: TimestampUnit::Nanos,
        }
    }
}

/// CSV 数据源
pub struct CsvSource {
    name: String,
    path: String,
    mapping: CsvColumnMapping,
    /// 显式 mapping 是否已设置(`with_mapping` 调用后为 true)
    explicit_mapping: bool,
    /// 缓存推断出的 mapping(首次 query 时填充)
    inferred: std::sync::OnceLock<CsvColumnMapping>,
}

impl CsvSource {
    /// 从文件路径构造(使用默认列映射,首次 query 时尝试从 header 推断)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axon_data::sources::CsvSource;
    /// use axon_data::DataSource;
    /// let src = CsvSource::new("btc", "/tmp/data.csv");
    /// assert_eq!(src.name(), "btc");
    /// ```
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            mapping: CsvColumnMapping::default(),
            explicit_mapping: false,
            inferred: std::sync::OnceLock::new(),
        }
    }

    /// 自定义列映射(builder 风格)
    pub fn with_mapping(mut self, mapping: CsvColumnMapping) -> Self {
        self.mapping = mapping;
        self.explicit_mapping = true; // 显式 mapping 优先于推断
        self
    }

    /// 获取最终生效的 mapping(显式 > 推断 > 默认)
    fn effective_mapping(&self) -> DataResult<CsvColumnMapping> {
        if self.explicit_mapping {
            return Ok(self.mapping);
        }
        if let Some(m) = self.inferred.get() {
            return Ok(*m);
        }
        // 首次:尝试从 header 推断
        let m = self.infer_mapping()?;
        // 若多个实例并发触发,get_or_init 更安全;这里用 OnceLock 直接 set,
        // 因为 infer_mapping 是幂等的(同一文件 + header 一定结果一致)
        let _ = self.inferred.set(m);
        Ok(m)
    }

    /// 推断列映射(读 header 行,按列名匹配到字段)
    fn infer_mapping(&self) -> DataResult<CsvColumnMapping> {
        let mut reader = csv::Reader::from_path(&self.path)
            .map_err(|e| DataError::InvalidRequest(format!("open {}: {}", self.path, e)))?;
        let headers = reader
            .headers()
            .map_err(|e| DataError::InvalidRequest(format!("read header: {e}")))?
            .clone();
        let mut mapping = CsvColumnMapping {
            timestamp_col: 0,
            price_col: 1,
            quantity_col: 2,
            side_col: None,
            timestamp_unit: TimestampUnit::Nanos,
        };
        for (i, h) in headers.iter().enumerate() {
            let h_lower = h.to_ascii_lowercase();
            match h_lower.as_str() {
                "timestamp" | "time" | "ts" | "datetime" | "date" => mapping.timestamp_col = i,
                "price" | "px" | "close" | "last" | "value" => mapping.price_col = i,
                "quantity" | "qty" | "size" | "volume" | "vol" => mapping.quantity_col = i,
                "side" | "buy_sell" | "direction" | "action" => mapping.side_col = Some(i),
                _ => {}
            }
        }
        Ok(mapping)
    }

    /// 按时间窗口过滤(纳秒为单位,包含两端)
    ///
    /// 复用 `query` 的解析逻辑,加载后过滤并重算 checksum。
    /// 主要用于延迟窗口裁剪 + 单元测试。
    pub async fn query_with_time_filter(
        &self,
        req: &DataRequest,
        start_nanos: i64,
        end_nanos: i64,
    ) -> DataResult<Dataset> {
        let mut ds = self.query(req).await?;
        ds.rows.retain(|t| {
            let ts = t.timestamp.nanos;
            ts >= start_nanos && ts <= end_nanos
        });
        // 重新计算 checksum(rows 改变后,原 checksum 失效)
        ds.checksum = Dataset::compute_checksum(&ds.rows);
        Ok(ds)
    }

    /// 读取 schema(用于 [`DataSource::schema`])
    fn schema_fields(&self) -> Vec<SchemaField> {
        vec![
            SchemaField { name: "timestamp".into(), dtype: crate::types::DataType::Timestamp },
            SchemaField { name: "price".into(), dtype: crate::types::DataType::F64 },
            SchemaField { name: "quantity".into(), dtype: crate::types::DataType::F64 },
            SchemaField { name: "side".into(), dtype: crate::types::DataType::String },
        ]
    }
}

#[async_trait]
impl DataSource for CsvSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn schema(&self) -> &[SchemaField] {
        // 简化:用 `let` 而非 `const`,因为 `String::from` 不是 const fn
        // 后续 PR3 改为 `static` + `Lazy` 减少 hot path 分配
        let schema: Vec<SchemaField> = self.schema_fields();
        Box::leak(schema.into_boxed_slice()) as &[SchemaField]
    }

    async fn query(&self, req: &DataRequest) -> DataResult<Dataset> {
        let m = self.effective_mapping()?;
        let mut reader = csv::Reader::from_path(&self.path)
            .map_err(|e| DataError::InvalidRequest(format!("open {}: {}", self.path, e)))?;

        let mut rows = Vec::new();
        for (i, record) in reader.records().enumerate() {
            let record = record.map_err(|e| {
                DataError::CorruptData {
                    expected: "valid csv row".into(),
                    actual: format!("line {i}: {e}"),
                    location: Some(CsvLocation { file: self.path.clone(), line: i + 2, column: None }),
                }
            })?;

            // 期望至少包含必填的 3 列(timestamp/price/quantity)
            let required = m.side_col.map(|s| s + 1).unwrap_or_else(|| m.quantity_col + 1);
            if record.len() < required {
                return Err(DataError::SchemaMismatch {
                    expected: format!("≥{required} columns"),
                    actual: format!("{} columns", record.len()),
                });
            }

            let raw_ts: i64 = record[m.timestamp_col].parse().map_err(|e| {
                DataError::CorruptData {
                    expected: format!("i64 timestamp (unit={:?})", m.timestamp_unit),
                    actual: format!("line {i}: {e}"),
                    location: Some(CsvLocation { file: self.path.clone(), line: i + 2, column: Some("timestamp".into()) }),
                }
            })?;
            let ts_nanos = m.timestamp_unit.to_nanos(raw_ts);

            let price: f64 = record[m.price_col].parse().map_err(|e| {
                DataError::CorruptData {
                    expected: "f64 price".into(),
                    actual: format!("line {i}: {e}"),
                    location: Some(CsvLocation { file: self.path.clone(), line: i + 2, column: Some("price".into()) }),
                }
            })?;
            let qty: f64 = record[m.quantity_col].parse().map_err(|e| {
                DataError::CorruptData {
                    expected: "f64 quantity".into(),
                    actual: format!("line {i}: {e}"),
                    location: Some(CsvLocation { file: self.path.clone(), line: i + 2, column: Some("quantity".into()) }),
                }
            })?;

            let side = if let Some(col) = m.side_col {
                match record[col].to_ascii_lowercase().as_str() {
                    "buy" | "b" => Side::Buy,
                    "sell" | "s" => Side::Sell,
                    other => {
                        return Err(DataError::CorruptData {
                            expected: "buy/sell".into(),
                            actual: format!("line {i}: '{other}'"),
                            location: Some(CsvLocation { file: self.path.clone(), line: i + 2, column: Some("side".into()) }),
                        })
                    }
                }
            } else {
                // 无 side 列时默认 Buy(简化)
                Side::Buy
            };

            let timestamp = Timestamp::from_nanos(ts_nanos);
            let price = Price::from_f64(price);
            let quantity = Quantity::from_f64(qty);

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
            self.schema_fields(),
            self.name.clone(),
            req.clone(),
        ))
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
    use super::*;
    use crate::types::DataRequest;
    use chrono::Utc;

    /// 构造测试用请求
    fn make_test_req() -> DataRequest {
        DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick)
    }

    #[test]
    fn timestamp_unit_to_nanos_uses_correct_multiplier() {
        assert_eq!(TimestampUnit::Nanos.to_nanos(1), 1);
        assert_eq!(TimestampUnit::Micros.to_nanos(2), 2_000);
        assert_eq!(TimestampUnit::Millis.to_nanos(3), 3_000_000);
        assert_eq!(TimestampUnit::Secs.to_nanos(4), 4_000_000_000);
    }

    #[test]
    fn default_mapping_matches_v0_columns() {
        let m = CsvColumnMapping::default();
        assert_eq!(m.timestamp_col, 0);
        assert_eq!(m.price_col, 1);
        assert_eq!(m.quantity_col, 2);
        assert_eq!(m.side_col, Some(3));
        assert_eq!(m.timestamp_unit, TimestampUnit::Nanos);
    }

    #[test]
    fn csv_source_with_custom_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom.csv");
        // 列序:ts,px,qty,side(nanos)
        std::fs::write(&path, "ts,px,qty,side\n100,1.0,10,buy\n200,2.0,20,sell\n").unwrap();
        let mapping = CsvColumnMapping {
            timestamp_col: 0,
            price_col: 1,
            quantity_col: 2,
            side_col: Some(3),
            timestamp_unit: TimestampUnit::Nanos,
        };
        let src = CsvSource::new("test", path.to_str().unwrap()).with_mapping(mapping);
        let req = make_test_req();
        let ds = futures::executor::block_on(src.query(&req)).unwrap();
        assert_eq!(ds.len(), 2);
    }

    #[test]
    fn csv_source_handles_millis_unit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("millis.csv");
        // 时间戳是毫秒:1,2,3 -> 纳秒 1e6, 2e6, 3e6
        std::fs::write(&path, "ts,px,qty,side\n1,10.0,1,buy\n2,20.0,1,sell\n3,30.0,1,buy\n").unwrap();
        let mapping = CsvColumnMapping {
            timestamp_col: 0,
            price_col: 1,
            quantity_col: 2,
            side_col: Some(3),
            timestamp_unit: TimestampUnit::Millis,
        };
        let src = CsvSource::new("test", path.to_str().unwrap()).with_mapping(mapping);
        let req = make_test_req();
        let ds = futures::executor::block_on(src.query(&req)).unwrap();
        assert_eq!(ds.len(), 3);
        // 验证时间戳被正确转换为纳秒
        let ts: Vec<i64> = ds.iter().map(|t| t.timestamp.nanos).collect();
        assert_eq!(ts, vec![1_000_000, 2_000_000, 3_000_000]);
    }

    #[test]
    fn csv_source_without_side_column_defaults_to_buy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_side.csv");
        // 3 列,无 side
        std::fs::write(&path, "ts,px,qty\n100,1.0,10\n200,2.0,20\n").unwrap();
        let mapping = CsvColumnMapping {
            timestamp_col: 0,
            price_col: 1,
            quantity_col: 2,
            side_col: None,
            timestamp_unit: TimestampUnit::Nanos,
        };
        let src = CsvSource::new("test", path.to_str().unwrap()).with_mapping(mapping);
        let req = make_test_req();
        let ds = futures::executor::block_on(src.query(&req)).unwrap();
        assert_eq!(ds.len(), 2);
        // 所有 tick 都是 Buy
        assert!(ds.iter().all(|t| t.side == Side::Buy));
    }

    #[test]
    fn csv_source_filters_by_time_window() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window.csv");
        // 5 行,纳秒时间戳:0, 1, 2, 3, 4
        std::fs::write(
            &path,
            "ts,px,qty,side\n0,100,1,buy\n1,101,1,buy\n2,102,1,buy\n3,103,1,buy\n4,104,1,buy\n",
        )
        .unwrap();
        let src = CsvSource::new("test", path.to_str().unwrap());
        let req = make_test_req();
        // 过滤窗口 [1, 3],保留 1, 2, 3
        let ds = futures::executor::block_on(
            src.query_with_time_filter(&req, 1, 3),
        )
        .unwrap();
        assert_eq!(ds.len(), 3);
        let ts: Vec<i64> = ds.iter().map(|t| t.timestamp.nanos).collect();
        assert_eq!(ts, vec![1, 2, 3]);
    }

    #[test]
    fn csv_source_time_filter_recomputes_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ck.csv");
        std::fs::write(&path, "ts,px,qty,side\n0,100,1,buy\n1,101,1,buy\n2,102,1,buy\n").unwrap();
        let src = CsvSource::new("test", path.to_str().unwrap());
        let req = make_test_req();
        let full = futures::executor::block_on(src.query(&req)).unwrap();
        let filtered = futures::executor::block_on(src.query_with_time_filter(&req, 0, 1)).unwrap();
        // 过滤后 checksum 必须不同(行数变了)
        assert_ne!(full.checksum, filtered.checksum);
        // 过滤后 checksum 等于用 2 行重算的值
        let expected = Dataset::compute_checksum(&full.rows[..2]);
        assert_eq!(filtered.checksum, expected);
    }

    #[test]
    fn csv_source_time_filter_empty_window() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty_window.csv");
        std::fs::write(&path, "ts,px,qty,side\n0,100,1,buy\n1,101,1,buy\n").unwrap();
        let src = CsvSource::new("test", path.to_str().unwrap());
        let req = make_test_req();
        // 窗口 [100, 200] 不在 [0, 1] 范围
        let ds = futures::executor::block_on(src.query_with_time_filter(&req, 100, 200)).unwrap();
        assert!(ds.is_empty());
    }

    #[test]
    fn csv_source_schema_inference_recognizes_renamed_columns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("renamed.csv");
        // 列名:time, close, volume, buy_sell - 推断器应识别
        std::fs::write(&path, "time,close,volume,buy_sell\n100,50.0,1,buy\n200,60.0,2,sell\n").unwrap();
        let src = CsvSource::new("test", path.to_str().unwrap());
        let req = make_test_req();
        let ds = futures::executor::block_on(src.query(&req)).unwrap();
        assert_eq!(ds.len(), 2);
        // price 应从第 2 列(close)读到
        let prices: Vec<f64> = ds.iter().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![50.0, 60.0]);
        // side 应从第 4 列(buy_sell)推断
        let sides: Vec<Side> = ds.iter().map(|t| t.side).collect();
        assert_eq!(sides, vec![Side::Buy, Side::Sell]);
    }

    #[test]
    fn csv_source_schema_inference_falls_back_to_defaults_on_unknown_headers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("unknown.csv");
        // 未知列名 + 位置仍按默认 0/1/2/3
        std::fs::write(&path, "a,b,c,d\n100,1.0,10,buy\n").unwrap();
        let src = CsvSource::new("test", path.to_str().unwrap());
        let req = make_test_req();
        let ds = futures::executor::block_on(src.query(&req)).unwrap();
        assert_eq!(ds.len(), 1);
        assert_eq!(ds.iter().next().unwrap().price.as_f64(), 1.0);
    }

    #[test]
    fn csv_source_explicit_mapping_overrides_inference() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("override.csv");
        // header 是 time/close,显式 mapping 指 timestamp_col=0 但 price_col=1
        // (与推断一致,这里验证显式 mapping 路径不报错)
        std::fs::write(&path, "time,close,volume\n100,1.0,10\n").unwrap();
        let mapping = CsvColumnMapping {
            timestamp_col: 0,
            price_col: 1,
            quantity_col: 2,
            side_col: None,
            timestamp_unit: TimestampUnit::Nanos,
        };
        let src = CsvSource::new("test", path.to_str().unwrap()).with_mapping(mapping);
        let req = make_test_req();
        let ds = futures::executor::block_on(src.query(&req)).unwrap();
        assert_eq!(ds.len(), 1);
        assert_eq!(ds.iter().next().unwrap().timestamp.nanos, 100);
        assert_eq!(ds.iter().next().unwrap().price.as_f64(), 1.0);
    }
}
