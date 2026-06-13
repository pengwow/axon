//! Arrow IPC 持久化模块
//!
//! 提供 Dataset / BarDataset 的 Arrow IPC 文件读写能力。
//!
//! ## 设计原则
//!
//! - `IpcWritable` trait 统一 Dataset 和 BarDataset 的写入接口
//! - `IpcWriter` 写入时在 schema metadata 中嵌入 `axon_data_type` 和 `axon_frequency`
//! - `IpcReader` 读取时根据 schema 列数校验类型(4 列 = Tick, 6 列 = Bar)

use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use std::sync::Arc;

use arrow::datatypes::Schema;
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;

use crate::bar::BarDataset;
use crate::dataset::Dataset;
use crate::error::{DataError, DataResult};
use crate::types::Frequency;

/// IPC 可写 trait(统一 Dataset 和 BarDataset 的写入接口)
pub trait IpcWritable {
    /// 获取 schema
    fn schema(&self) -> &Arc<Schema>;
    /// 获取 batches
    fn batches(&self) -> &[RecordBatch];
    /// 获取数据源名称
    fn source(&self) -> &str;
    /// 获取校验和
    fn checksum(&self) -> &str;
    /// 返回频率标签(BarDataset 返回 Some("Min1")，Dataset 返回 None)
    fn frequency_tag(&self) -> Option<String> {
        None
    }
}

/// 为 Tick Dataset 实现 IpcWritable
impl IpcWritable for Dataset {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }
    fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }
    fn source(&self) -> &str {
        &self.source
    }
    fn checksum(&self) -> &str {
        &self.checksum
    }
}

/// 为 BarDataset 实现 IpcWritable
impl IpcWritable for BarDataset {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }
    fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }
    fn source(&self) -> &str {
        &self.source
    }
    fn checksum(&self) -> &str {
        &self.checksum
    }
    fn frequency_tag(&self) -> Option<String> {
        Some(self.frequency.as_str().to_string())
    }
}

/// 根据 schema 列数判断数据类型标签
fn data_type_tag(data: &dyn IpcWritable) -> &'static str {
    if data.schema().fields().len() == 4 {
        "tick"
    } else {
        "bar"
    }
}

/// IPC 写入器
pub struct IpcWriter;

impl IpcWriter {
    /// 将数据写入 Arrow IPC 文件(.arrow)
    pub fn write<W: Write + Seek>(writer: W, data: &dyn IpcWritable) -> DataResult<()> {
        let schema = data.schema().clone();
        // 在 schema metadata 中嵌入 data_type + frequency 标记
        let mut metadata = schema.metadata().clone();
        metadata.insert("axon_data_type".into(), data_type_tag(data).to_string());
        metadata.insert("axon_source".into(), data.source().to_string());
        if let Some(freq) = data.frequency_tag() {
            metadata.insert("axon_frequency".into(), freq);
        }
        let fields: Vec<_> = schema.fields().iter().map(|f| f.as_ref().clone()).collect();
        let schema_with_meta = Arc::new(Schema::new_with_metadata(fields, metadata));

        let mut writer = FileWriter::try_new(writer, &schema_with_meta)
            .map_err(|e| DataError::Internal(format!("IPC writer init: {e}")))?;
        for batch in data.batches() {
            writer
                .write(batch)
                .map_err(|e| DataError::Internal(format!("IPC write batch: {e}")))?;
        }
        writer
            .finish()
            .map_err(|e| DataError::Internal(format!("IPC finish: {e}")))?;
        Ok(())
    }

    /// 便捷方法:写入文件路径
    pub fn write_to_path(path: impl AsRef<Path>, data: &dyn IpcWritable) -> DataResult<()> {
        let file = File::create(path)?;
        Self::write(file, data)
    }
}

/// IPC 读取器
pub struct IpcReader;

impl IpcReader {
    /// 读取 IPC 文件为 Tick Dataset(校验 4 列 schema)
    pub fn read_tick(path: impl AsRef<Path>) -> DataResult<Dataset> {
        let (schema, batches) = Self::read_batches_inner(path)?;
        // 校验 4 列 Tick schema
        if schema.fields().len() != 4 {
            return Err(DataError::IpcSchemaMismatch {
                expected: 4,
                actual: schema.fields().len(),
                expected_type: "tick".into(),
            });
        }
        let source = schema
            .metadata()
            .get("axon_source")
            .cloned()
            .unwrap_or_default();
        // 从 metadata 恢复 request 信息(简化版，只保留 symbol)
        let req = crate::types::DataRequest::new(
            &source,
            chrono::Utc::now(),
            chrono::Utc::now(),
            crate::types::Frequency::Tick,
        );
        Dataset::new(batches, source, req)
    }

    /// 读取 IPC 文件为 BarDataset
    /// 校验:6 列 schema + schema metadata 中 axon_frequency 标记
    pub fn read_bar(path: impl AsRef<Path>) -> DataResult<BarDataset> {
        let (schema, batches) = Self::read_batches_inner(path)?;
        // 校验 6 列 Bar schema
        if schema.fields().len() != 6 {
            return Err(DataError::IpcSchemaMismatch {
                expected: 6,
                actual: schema.fields().len(),
                expected_type: "bar".into(),
            });
        }
        let source = schema
            .metadata()
            .get("axon_source")
            .cloned()
            .unwrap_or_default();
        let freq_str = schema
            .metadata()
            .get("axon_frequency")
            .cloned()
            .unwrap_or_default();
        // 匹配 Frequency::as_str() 的输出格式
        let frequency = match freq_str.as_str() {
            "1m" => Frequency::Min1,
            "5m" => Frequency::Min5,
            "15m" => Frequency::Min15,
            "30m" => Frequency::Min30,
            "1h" => Frequency::Hour1,
            "4h" => Frequency::Hour4,
            "1d" => Frequency::Day1,
            "1w" => Frequency::Week1,
            "1M" => Frequency::Month1,
            _ => {
                return Err(DataError::InvalidRequest(format!(
                    "unknown frequency in IPC metadata: {freq_str}"
                )))
            }
        };
        let req = crate::types::DataRequest::new(
            &source,
            chrono::Utc::now(),
            chrono::Utc::now(),
            frequency,
        );
        BarDataset::new(batches, source, req, frequency)
    }

    /// 读取 IPC 文件为通用 RecordBatch 列表(不关心具体类型)
    pub fn read_batches(
        path: impl AsRef<Path>,
    ) -> DataResult<(Arc<Schema>, Vec<RecordBatch>)> {
        Self::read_batches_inner(path)
    }

    /// 内部实现:读取 IPC 文件
    fn read_batches_inner(
        path: impl AsRef<Path>,
    ) -> DataResult<(Arc<Schema>, Vec<RecordBatch>)> {
        let file = File::open(path)?;
        let reader = FileReader::try_new(file, None)
            .map_err(|e| DataError::Internal(format!("IPC reader init: {e}")))?;
        let schema = Arc::new(reader.schema().as_ref().clone());
        let mut batches = Vec::new();
        for batch_result in reader {
            let batch = batch_result
                .map_err(|e| DataError::Internal(format!("IPC read batch: {e}")))?;
            batches.push(batch);
        }
        Ok((schema, batches))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar::BarDataset;
    use crate::dataset::Dataset;
    use crate::types::{DataRequest, Frequency};
    use axon_core::market::{Bar, Side, Tick};
    use axon_core::time::Timestamp;
    use axon_core::types::{Price, Quantity};
    use chrono::Utc;
    use tempfile::NamedTempFile;

    fn make_tick(nanos: i64, price: f64) -> Tick {
        Tick::new(
            Timestamp::from_nanos(nanos),
            Price::from_f64(price),
            Quantity::from_f64(1.0),
            Side::Buy,
        )
    }

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

    #[test]
    fn ipc_tick_roundtrip() {
        let ticks: Vec<Tick> = (0..10)
            .map(|i| make_tick(i * 1_000_000_000, 100.0 + i as f64))
            .collect();
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = Dataset::from_ticks(ticks, "test".into(), req).unwrap();
        let original_checksum = ds.checksum.clone();

        let tmp = NamedTempFile::new().unwrap();
        IpcWriter::write_to_path(tmp.path(), &ds).unwrap();

        let loaded = IpcReader::read_tick(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 10);
        assert_eq!(loaded.checksum, original_checksum);
    }

    #[test]
    fn ipc_bar_roundtrip() {
        let bars: Vec<Bar> = (0..5)
            .map(|i| make_bar(i * 60_000_000_000, 100.0, 110.0, 90.0, 105.0, 100.0))
            .collect();
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Min1);
        let ds = BarDataset::from_bars(bars, "test".into(), req, Frequency::Min1).unwrap();
        let original_checksum = ds.checksum.clone();

        let tmp = NamedTempFile::new().unwrap();
        IpcWriter::write_to_path(tmp.path(), &ds).unwrap();

        let loaded = IpcReader::read_bar(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 5);
        assert_eq!(loaded.checksum, original_checksum);
        assert_eq!(loaded.frequency(), Frequency::Min1);
    }

    #[test]
    fn ipc_bar_file_as_tick_returns_schema_mismatch() {
        let bars = vec![make_bar(0, 100.0, 110.0, 90.0, 105.0, 100.0)];
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Min1);
        let ds = BarDataset::from_bars(bars, "test".into(), req, Frequency::Min1).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        IpcWriter::write_to_path(tmp.path(), &ds).unwrap();

        let result = IpcReader::read_tick(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn ipc_read_batches_returns_raw() {
        let ticks: Vec<Tick> = (0..5)
            .map(|i| make_tick(i * 1_000_000_000, 100.0))
            .collect();
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = Dataset::from_ticks(ticks, "test".into(), req).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        IpcWriter::write_to_path(tmp.path(), &ds).unwrap();

        let (schema, batches) = IpcReader::read_batches(tmp.path()).unwrap();
        assert_eq!(schema.fields().len(), 4);
        assert_eq!(
            batches.iter().map(|b| b.num_rows()).sum::<usize>(),
            5
        );
    }

    #[test]
    fn ipc_empty_dataset_roundtrip() {
        let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = Dataset::from_ticks(vec![], "test".into(), req).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        IpcWriter::write_to_path(tmp.path(), &ds).unwrap();

        let loaded = IpcReader::read_tick(tmp.path()).unwrap();
        assert!(loaded.is_empty());
    }
}
