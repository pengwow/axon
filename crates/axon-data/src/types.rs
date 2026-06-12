//! 核心数据类型
//!
//! - [`DataRequest`]:数据查询请求(单 symbol + 时间窗口 + 频率)
//! - [`Frequency`]:数据频率枚举(Tick / 1m / 5m / ...)
//! - [`SchemaField`]:数据 schema 字段定义(轻量版,无 Arrow 依赖)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── 频率枚举 ──────────────────────────────────────────────

/// 数据频率
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Frequency {
    /// 逐笔成交
    Tick,
    /// 1 分钟
    Min1,
    /// 5 分钟
    Min5,
    /// 15 分钟
    Min15,
    /// 30 分钟
    Min30,
    /// 1 小时
    Hour1,
    /// 4 小时
    Hour4,
    /// 1 天
    Day1,
    /// 1 周
    Week1,
    /// 1 月
    Month1,
}

impl Frequency {
    /// 序列化为字符串(对外协议友好)
    pub fn as_str(&self) -> &'static str {
        match self {
            Frequency::Tick => "tick",
            Frequency::Min1 => "1m",
            Frequency::Min5 => "5m",
            Frequency::Min15 => "15m",
            Frequency::Min30 => "30m",
            Frequency::Hour1 => "1h",
            Frequency::Hour4 => "4h",
            Frequency::Day1 => "1d",
            Frequency::Week1 => "1w",
            Frequency::Month1 => "1M",
        }
    }

    /// 是否为 K 线频率(非 Tick)
    pub fn is_bar(&self) -> bool {
        !matches!(self, Frequency::Tick)
    }
}

// ─── 数据请求 ──────────────────────────────────────────────

/// 数据查询请求
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DataRequest {
    /// 标的符号,如 "BTCUSDT" / "AAPL"
    pub symbol: String,
    /// 起始时间(UTC,包含)
    pub start: DateTime<Utc>,
    /// 结束时间(UTC,包含)
    pub end: DateTime<Utc>,
    /// 数据频率
    pub frequency: Frequency,
    /// 字段子集(空 = 全部)
    pub fields: Vec<String>,
    /// 指定数据源名称;None = 自动选择
    pub source: Option<String>,
}

impl DataRequest {
    /// 构造基础请求(默认全字段、自动选源)
    pub fn new(
        symbol: impl Into<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        frequency: Frequency,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            start,
            end,
            frequency,
            fields: Vec::new(),
            source: None,
        }
    }

    /// 设置字段子集(builder 风格)
    pub fn with_fields(mut self, fields: Vec<String>) -> Self {
        self.fields = fields;
        self
    }

    /// 指定数据源(builder 风格)
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// 时间窗口是否合法(start <= end)
    pub fn is_valid(&self) -> bool {
        self.start <= self.end
    }
}

// ─── Schema 字段(轻量版)───────────────────────────────────

/// Schema 字段定义(简化版,避免 Arrow 依赖)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaField {
    /// 字段名
    pub name: String,
    /// 字段类型
    pub dtype: DataType,
}

impl SchemaField {
    /// 构造新字段
    pub fn new(name: impl Into<String>, dtype: DataType) -> Self {
        Self {
            name: name.into(),
            dtype,
        }
    }
}

/// 数据类型(简化版,只支持常见基础类型)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// 64 位浮点
    F64,
    /// 64 位整数
    I64,
    /// 布尔
    Bool,
    /// 字符串
    String,
    /// 时间戳(纳秒)
    Timestamp,
}
