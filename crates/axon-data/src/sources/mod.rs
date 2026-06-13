//! 数据源实现
//!
//! - `MockSource`:测试用 mock 数据源(总是可用,默认公开)
//! - `CsvSource`:CSV 文件源(需 `csv-source` feature)
//! - `ParquetSource`:Parquet 列式文件源(需 `parquet-source` feature,PR3 M2)
//! - 后续:WebSocket / Replay / Synthetic(feature-gated)

#[cfg(feature = "csv-source")]
pub mod csv;

#[cfg(feature = "parquet-source")]
pub mod parquet;

pub mod mock;

pub use mock::MockSource;

#[cfg(feature = "csv-source")]
pub use csv::{CsvColumnMapping, CsvSource, TimestampUnit};

#[cfg(feature = "parquet-source")]
pub use parquet::ParquetSource;
