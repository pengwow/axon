//! 日报生成器
//!
//! 从交易记录生成日报，计算日内盈亏、胜率、费用等指标。

use chrono::NaiveDate;

use crate::report::{DailyReport, FeeBreakdown};
use crate::types::TradeRecord;

/// 日报生成器
pub struct DailyReportGenerator;

impl DailyReportGenerator {
    /// 生成日报
    ///
    /// # 参数
    /// - `date`: 报告日期
    /// - `account_id`: 账户 ID
    /// - `starting_balance`: 期初余额
    /// - `trades`: 当日交易记录（按日期过滤后）
    /// - `fee_currency`: 费用货币
    pub fn generate(
        date: NaiveDate,
        account_id: &str,
        starting_balance: f64,
        trades: &[&TradeRecord],
        fee_currency: &str,
    ) -> DailyReport {
        // 计算已实现盈亏
        let realized_pnl: f64 = trades.iter().filter_map(|t| t.realized_pnl).sum();

        // 计算总手续费
        let total_fees: f64 = trades.iter().map(|t| t.fee).sum();

        // 统计盈亏交易
        let winning_trades = trades
            .iter()
            .filter(|t| t.realized_pnl.unwrap_or(0.0) > 0.0)
            .count() as u32;

        let losing_trades = trades
            .iter()
            .filter(|t| t.realized_pnl.unwrap_or(0.0) < 0.0)
            .count() as u32;

        // 计算最大单笔盈亏
        let largest_win = trades
            .iter()
            .filter_map(|t| t.realized_pnl)
            .filter(|p| *p > 0.0)
            .fold(0.0f64, f64::max);

        let largest_loss = trades
            .iter()
            .filter_map(|t| t.realized_pnl)
            .filter(|p| *p < 0.0)
            .fold(0.0f64, f64::min);

        // 费用明细
        let fee_breakdown = FeeBreakdown {
            trading_fees: total_fees,
            funding_fees: 0.0,
            withdrawal_fees: 0.0,
            other_fees: 0.0,
            total: total_fees,
            fee_currency: fee_currency.into(),
        };

        // 期末余额 = 期初 + 已实现盈亏 - 手续费
        let ending_balance = starting_balance + realized_pnl - total_fees;

        DailyReport {
            date,
            account_id: account_id.into(),
            starting_balance,
            ending_balance,
            net_pnl: realized_pnl - total_fees,
            realized_pnl,
            unrealized_pnl: 0.0,
            total_fees,
            total_trades: trades.len() as u32,
            winning_trades,
            losing_trades,
            largest_win,
            largest_loss,
            positions: vec![],
            fee_breakdown,
            trade_count: trades.len() as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LiquidityType, OrderType, TradeSide, TradeStatus};
    use chrono::Utc;
    use uuid::Uuid;

    /// 创建测试交易
    fn make_trade(
        symbol: &str,
        side: TradeSide,
        quantity: f64,
        price: f64,
        realized_pnl: Option<f64>,
        fee: f64,
    ) -> TradeRecord {
        TradeRecord {
            trade_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            strategy_id: "test".into(),
            symbol: symbol.into(),
            side,
            quantity,
            price,
            notional_value: quantity * price,
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
    fn test_daily_report_with_trades() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let trades = [
            make_trade("BTCUSDT", TradeSide::Buy, 1.0, 50000.0, Some(500.0), 50.0),
            make_trade("ETHUSDT", TradeSide::Sell, 10.0, 3000.0, Some(-200.0), 30.0),
        ];
        let trade_refs: Vec<&TradeRecord> = trades.iter().collect();

        let report = DailyReportGenerator::generate(date, "test", 100000.0, &trade_refs, "USDT");

        // 验证计算正确性
        assert_eq!(report.total_trades, 2);
        assert_eq!(report.winning_trades, 1);
        assert_eq!(report.losing_trades, 1);
        assert!((report.realized_pnl - 300.0).abs() < f64::EPSILON);
        assert!((report.total_fees - 80.0).abs() < f64::EPSILON);
        assert!((report.net_pnl - 220.0).abs() < f64::EPSILON);
        assert!((report.ending_balance - 100220.0).abs() < f64::EPSILON);
        assert!((report.largest_win - 500.0).abs() < f64::EPSILON);
        assert!((report.largest_loss - (-200.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_empty_day_report() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let trades: Vec<&TradeRecord> = vec![];

        let report = DailyReportGenerator::generate(date, "test", 100000.0, &trades, "USDT");

        // 空交易日应返回零值报告
        assert_eq!(report.total_trades, 0);
        assert_eq!(report.winning_trades, 0);
        assert_eq!(report.losing_trades, 0);
        assert!((report.realized_pnl).abs() < f64::EPSILON);
        assert!((report.total_fees).abs() < f64::EPSILON);
        assert!((report.ending_balance - 100000.0).abs() < f64::EPSILON);
    }
}
