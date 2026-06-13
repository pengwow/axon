//! 监管报送模块
//!
//! 提供监管指标计算、报送生成和格式导出功能。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::TradeId;

/// 监管报送数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatorySubmission {
    /// 报送 ID
    pub submission_id: Uuid,
    /// 监管机构
    pub regulator: String,
    /// 报送类型
    pub submission_type: SubmissionType,
    /// 报送周期开始时间
    pub period_start: DateTime<Utc>,
    /// 报送周期结束时间
    pub period_end: DateTime<Utc>,
    /// 监管数据
    pub data: RegulatoryData,
    /// 输出格式
    pub format: RegulatorFormat,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
    /// 提交时间
    pub submitted_at: Option<DateTime<Utc>>,
}

/// 报送类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubmissionType {
    /// 日报
    Daily,
    /// 周报
    Weekly,
    /// 月报
    Monthly,
    /// 季报
    Quarterly,
    /// 年报
    Annual,
    /// 事件驱动
    EventDriven,
}

/// 监管数据（仅前 4 项指标）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatoryData {
    /// 总成交额
    pub total_turnover: f64,
    /// 持仓限制检查
    pub position_limits: Vec<PositionLimit>,
    /// 集中度检查
    pub concentration_limits: Vec<ConcentrationCheck>,
    /// 大额交易报告
    pub large_trade_reports: Vec<LargeTradeReport>,
}

/// 持仓限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionLimit {
    /// 交易对
    pub symbol: String,
    /// 当前持仓
    pub current_position: f64,
    /// 限制值
    pub limit: f64,
    /// 使用率（百分比）
    pub utilization_pct: f64,
    /// 是否违规
    pub breach: bool,
}

/// 集中度检查
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentrationCheck {
    /// 类别（通常是交易对）
    pub category: String,
    /// 敞口
    pub exposure: f64,
    /// 限制值
    pub limit: f64,
    /// 使用率（百分比）
    pub utilization_pct: f64,
    /// 是否违规
    pub breach: bool,
}

/// 大额交易报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeTradeReport {
    /// 交易 ID
    pub trade_id: TradeId,
    /// 交易对
    pub symbol: String,
    /// 名义价值
    pub notional_value: f64,
    /// 阈值
    pub threshold: f64,
    /// 是否需要报告
    pub requires_report: bool,
}

/// 监管格式（仅 JSON/CSV）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegulatorFormat {
    /// JSON 格式
    JSON,
    /// CSV 格式
    CSV,
}

/// 指标计算模块
pub mod metrics;
/// 报送生成模块
pub mod submission;
