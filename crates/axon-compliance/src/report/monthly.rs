//! 月报生成器
//!
//! 从交易记录聚合生成月报，计算月度统计指标。

use crate::error::{ComplianceError, ComplianceResult};
use crate::report::{MonthlyReport, PositionsSummary};
use crate::types::TradeRecord;

/// 月报生成器
pub struct MonthlyReportGenerator;

impl MonthlyReportGenerator {
    /// 从交易记录生成月报
    ///
    /// # 参数
    /// - `year`: 年份
    /// - `month`: 月份（1-12）
    /// - `account_id`: 账户 ID
    /// - `trades`: 当月所有交易
    /// - `active_days`: 活跃交易日数
    pub fn generate(
        year: u32,
        month: u32,
        account_id: &str,
        trades: &[&TradeRecord],
        active_days: u32,
    ) -> ComplianceResult<MonthlyReport> {
        if !(1..=12).contains(&month) {
            return Err(ComplianceError::ReportError("月份必须在 1-12 之间".into()));
        }

        // 计算总盈亏
        let total_pnl: f64 = trades.iter().filter_map(|t| t.realized_pnl).sum();

        // 计算总手续费
        let total_fees: f64 = trades.iter().map(|t| t.fee).sum();

        // 统计盈亏交易
        let winning_trades = trades
            .iter()
            .filter(|t| t.realized_pnl.unwrap_or(0.0) > 0.0)
            .count() as u32;

        let total_trades = trades.len() as u32;

        // 计算胜率
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };

        // 计算日均盈亏
        let avg_daily_pnl = if active_days > 0 {
            total_pnl / active_days as f64
        } else {
            0.0
        };

        // 计算最大单日亏损（简化：取所有负盈亏的最小值）
        let max_daily_loss = trades
            .iter()
            .filter_map(|t| t.realized_pnl)
            .filter(|p| *p < 0.0)
            .fold(0.0f64, f64::min);

        // 简化版夏普比率
        let pnl_values: Vec<f64> = trades.iter().filter_map(|t| t.realized_pnl).collect();
        let sharpe_ratio = compute_sharpe_ratio(&pnl_values);

        // 收集交易过的资产
        let mut assets_traded: Vec<String> = trades
            .iter()
            .map(|t| t.symbol.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        assets_traded.sort();

        // 持仓汇总
        let positions_summary = PositionsSummary {
            total_positions: assets_traded.len() as u32,
            avg_holding_period_hours: 0.0,
            max_concurrent_positions: 0,
            assets_traded,
        };

        Ok(MonthlyReport {
            year,
            month,
            account_id: account_id.into(),
            total_pnl,
            total_fees,
            total_trades,
            win_rate,
            avg_daily_pnl,
            max_daily_loss,
            sharpe_ratio,
            max_drawdown: 0.0,
            active_days,
            positions_summary,
        })
    }
}

/// 计算简化版夏普比率（无风险利率假设为 0）
pub(crate) fn compute_sharpe_ratio(returns: &[f64]) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }

    let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance: f64 =
        returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;

    let std_dev = variance.sqrt();

    if std_dev < f64::EPSILON {
        return 0.0;
    }

    mean / std_dev
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LiquidityType, OrderType, TradeSide, TradeStatus};
    use chrono::Utc;
    use uuid::Uuid;

    fn make_trade(realized_pnl: Option<f64>, fee: f64) -> TradeRecord {
        TradeRecord {
            trade_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            strategy_id: "test".into(),
            symbol: "BTCUSDT".into(),
            side: TradeSide::Buy,
            quantity: 1.0,
            price: 50000.0,
            notional_value: 50000.0,
            fee,
            fee_currency: "USDT".into(),
            exchange: "Binance".into(),
            execution_time: Utc::now(),
            settlement_time: None,
            status: TradeStatus::Filled,
            order_type: OrderType::Market,
            exchange_trade_id: None,
            liquidity: LiquidityType::Taker,
            realized_pnl,
            funding_rate: None,
            slippage: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_monthly_report_generation() {
        let trades = [
            make_trade(Some(500.0), 50.0),
            make_trade(Some(-200.0), 30.0),
            make_trade(Some(100.0), 20.0),
        ];
        let trade_refs: Vec<&TradeRecord> = trades.iter().collect();

        let report = MonthlyReportGenerator::generate(2026, 6, "test", &trade_refs, 20).unwrap();

        assert_eq!(report.year, 2026);
        assert_eq!(report.month, 6);
        assert_eq!(report.total_trades, 3);
        assert!((report.total_pnl - 400.0).abs() < f64::EPSILON);
        assert!((report.total_fees - 100.0).abs() < f64::EPSILON);
        assert!((report.win_rate - (2.0 / 3.0)).abs() < 0.001);
        assert!((report.avg_daily_pnl - 20.0).abs() < f64::EPSILON);
        assert!((report.max_daily_loss - (-200.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_empty_monthly_report() {
        let trades: Vec<&TradeRecord> = vec![];
        let report = MonthlyReportGenerator::generate(2026, 6, "test", &trades, 0).unwrap();

        assert_eq!(report.total_trades, 0);
        assert!((report.total_pnl).abs() < f64::EPSILON);
        assert!((report.win_rate).abs() < f64::EPSILON);
        assert!((report.avg_daily_pnl).abs() < f64::EPSILON);
    }

    #[test]
    fn test_invalid_month() {
        let trades: Vec<&TradeRecord> = vec![];
        let result = MonthlyReportGenerator::generate(2026, 13, "test", &trades, 0);
        assert!(result.is_err());
    }
}
