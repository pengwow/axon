//! 数据服务统一错误类型
//!
//! 设计原则:thiserror derive,5 个核心变体,支持 `?` 自动转换。

use thiserror::Error;

/// CSV 数据源错误位置(文件:行:列)
///
/// 用于 `DataError::CorruptData` 等变体的可选位置上下文,
/// 方便用户定位出错的具体文件/行/列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvLocation {
    /// 文件名或路径
    pub file: String,
    /// 1-indexed 行号
    pub line: usize,
    /// 列名(可选)
    pub column: Option<String>,
}

impl std::fmt::Display for CsvLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.column {
            Some(col) => write!(f, "{}:{}:{}", self.file, self.line, col),
            None => write!(f, "{}:{}", self.file, self.line),
        }
    }
}

/// 数据服务错误
#[derive(Debug, Error)]
pub enum DataError {
    /// 数据源未找到
    #[error("data source not found: {0}")]
    SourceNotFound(String),

    /// Schema 不匹配
    #[error("schema mismatch: expected {expected}, got {actual}")]
    SchemaMismatch {
        /// 期望的 schema 描述
        expected: String,
        /// 实际遇到的 schema 描述
        actual: String,
    },

    /// 网络错误(reqwest 转换)
    #[error("network error: {0}")]
    Network(String),

    /// 数据损坏(checksum 校验失败 / schema 不一致)
    #[error("corrupt data: {expected} != {actual}{}", location.as_ref().map(|l| format!(" at {l}")).unwrap_or_default())]
    CorruptData {
        /// 期望的 SHA256
        expected: String,
        /// 实际的 SHA256
        actual: String,
        /// 可选的位置上下文(文件/行/列)
        location: Option<CsvLocation>,
    },

    /// 限流(server 提示重试)
    #[error("rate limited, retry after {retry_after_ms}ms")]
    RateLimited {
        /// 建议重试等待时间(毫秒)
        retry_after_ms: u64,
    },

    /// 非法请求参数
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// I/O 错误(std::io 转换)
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// 内部错误(Arrow / IO 转换失败、不可恢复的内部状态)
    #[error("internal error: {0}")]
    Internal(String),

    /// 不支持的频率(如 Frequency::Tick 用于 Bar 聚合)
    #[error("unsupported frequency: {0}")]
    UnsupportedFrequency(String),

    /// IPC 文件 schema 不匹配
    #[error("IPC schema mismatch: expected {expected}-column {expected_type}, got {actual}-column")]
    IpcSchemaMismatch {
        /// 期望的列数
        expected: usize,
        /// 实际的列数
        actual: usize,
        /// 期望的数据类型(tick/bar)
        expected_type: String,
    },
}

/// 统一 Result 类型别名
pub type DataResult<T> = Result<T, DataError>;

// ===== 测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_location_displays_with_line_and_column() {
        let loc = CsvLocation {
            file: "data.csv".into(),
            line: 42,
            column: Some("price".into()),
        };
        assert_eq!(format!("{loc}"), "data.csv:42:price");
    }

    #[test]
    fn csv_location_displays_without_column() {
        let loc = CsvLocation {
            file: "x.csv".into(),
            line: 1,
            column: None,
        };
        assert_eq!(format!("{loc}"), "x.csv:1");
    }

    #[test]
    fn corrupt_data_displays_with_location() {
        let loc = CsvLocation {
            file: "x.csv".into(),
            line: 5,
            column: None,
        };
        let err = DataError::CorruptData {
            expected: "f64".into(),
            actual: "NaN".into(),
            location: Some(loc),
        };
        let s = format!("{err}");
        assert!(s.contains("x.csv:5"));
        assert!(s.contains("f64"));
        assert!(s.contains("NaN"));
    }
}
