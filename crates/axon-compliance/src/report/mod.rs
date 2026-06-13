//! 报告生成模块
//!
//! 支持日报、月报、年报生成，以及 JSON/CSV 格式导出。

pub mod annual;
pub mod daily;
pub mod formatter;
pub mod monthly;

pub use formatter::ReportExporter;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::types::TradeSide;

/// 报告导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// JSON 格式
    JSON,
    /// CSV 格式
    CSV,
}

/// 日报
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyReport {
    /// 报告日期
    pub date: NaiveDate,
    /// 账户 ID
    pub account_id: String,
    /// 期初余额
    pub starting_balance: f64,
    /// 期末余额
    pub ending_balance: f64,
    /// 净盈亏
    pub net_pnl: f64,
    /// 已实现盈亏
    pub realized_pnl: f64,
    /// 未实现盈亏
    pub unrealized_pnl: f64,
    /// 总手续费
    pub total_fees: f64,
    /// 总交易数
    pub total_trades: u32,
    /// 盈利交易数
    pub winning_trades: u32,
    /// 亏损交易数
    pub losing_trades: u32,
    /// 最大单笔盈利
    pub largest_win: f64,
    /// 最大单笔亏损
    pub largest_loss: f64,
    /// 持仓快照
    pub positions: Vec<PositionSnapshot>,
    /// 费用明细
    pub fee_breakdown: FeeBreakdown,
    /// 交易计数
    pub trade_count: u32,
}

/// 月报
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyReport {
    /// 年份
    pub year: u32,
    /// 月份
    pub month: u32,
    /// 账户 ID
    pub account_id: String,
    /// 总盈亏
    pub total_pnl: f64,
    /// 总手续费
    pub total_fees: f64,
    /// 总交易数
    pub total_trades: u32,
    /// 胜率
    pub win_rate: f64,
    /// 日均盈亏
    pub avg_daily_pnl: f64,
    /// 最大单日亏损
    pub max_daily_loss: f64,
    /// 夏普比率（简化版）
    pub sharpe_ratio: f64,
    /// 最大回撤
    pub max_drawdown: f64,
    /// 活跃交易日数
    pub active_days: u32,
    /// 持仓汇总
    pub positions_summary: PositionsSummary,
}

/// 年报
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnualReport {
    /// 年份
    pub year: u32,
    /// 账户 ID
    pub account_id: String,
    /// 总回报
    pub total_return: f64,
    /// 年化回报率（百分比）
    pub annual_return_pct: f64,
    /// 总手续费
    pub total_fees: f64,
    /// 总交易数
    pub total_trades: u32,
    /// 胜率
    pub win_rate: f64,
    /// 夏普比率
    pub sharpe_ratio: f64,
    /// 最大回撤
    pub max_drawdown: f64,
    /// 合规评分（0-100）
    pub compliance_score: f64,
    /// 监管备注
    pub regulatory_notes: Vec<String>,
    /// 活跃月数
    pub active_months: u32,
}

/// 持仓快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSnapshot {
    /// 交易对
    pub symbol: String,
    /// 数量
    pub quantity: f64,
    /// 平均入场价
    pub avg_entry_price: f64,
    /// 当前价格
    pub current_price: f64,
    /// 市值
    pub market_value: f64,
    /// 未实现盈亏
    pub unrealized_pnl: f64,
    /// 未实现盈亏百分比
    pub unrealized_pnl_pct: f64,
    /// 仓位权重
    pub weight: f64,
    /// 方向
    pub side: TradeSide,
}

/// 费用明细
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeBreakdown {
    /// 交易手续费
    pub trading_fees: f64,
    /// 资金费率
    pub funding_fees: f64,
    /// 提现手续费
    pub withdrawal_fees: f64,
    /// 其他费用
    pub other_fees: f64,
    /// 总费用
    pub total: f64,
    /// 费用货币
    pub fee_currency: String,
}

/// 持仓汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionsSummary {
    /// 总持仓数
    pub total_positions: u32,
    /// 平均持仓时长（小时）
    pub avg_holding_period_hours: f64,
    /// 最大同时持仓数
    pub max_concurrent_positions: u32,
    /// 交易过的资产
    pub assets_traded: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_format_equality() {
        assert_eq!(ReportFormat::JSON, ReportFormat::JSON);
        assert_ne!(ReportFormat::JSON, ReportFormat::CSV);
    }

    #[test]
    fn test_daily_report_serialization() {
        let report = DailyReport {
            date: NaiveDate::from_ymd_opt(2026, 6, 13).unwrap(),
            account_id: "test".into(),
            starting_balance: 100000.0,
            ending_balance: 100500.0,
            net_pnl: 500.0,
            realized_pnl: 500.0,
            unrealized_pnl: 0.0,
            total_fees: 50.0,
            total_trades: 10,
            winning_trades: 6,
            losing_trades: 4,
            largest_win: 200.0,
            largest_loss: -100.0,
            positions: vec![],
            fee_breakdown: FeeBreakdown {
                trading_fees: 50.0,
                funding_fees: 0.0,
                withdrawal_fees: 0.0,
                other_fees: 0.0,
                total: 50.0,
                fee_currency: "USDT".into(),
            },
            trade_count: 10,
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: DailyReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.date, report.date);
        assert_eq!(deserialized.net_pnl, report.net_pnl);
    }
}
