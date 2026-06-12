//! 数据源实现
//!
//! - [`MockSource`]:测试用 mock 数据源(总是可用,默认公开)
//! - [`CsvSource`]:CSV 文件源(需 `csv-source` feature)
//! - 后续:Parquet / WebSocket / Replay / Synthetic(feature-gated)

#[cfg(feature = "csv-source")]
pub mod csv;

pub mod mock;

pub use mock::MockSource;

#[cfg(feature = "csv-source")]
pub use csv::{CsvColumnMapping, CsvSource, TimestampUnit};
