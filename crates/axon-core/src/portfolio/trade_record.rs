//! 交易记录

use serde::{Deserialize, Serialize};

use crate::market::Trade;

/// 交易记录（用于审计和盈亏计算）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeRecord {
    /// 成交记录
    pub trade: Trade,
    /// 已实现盈亏（单位：f64 × 1e6）
    pub realized_pnl: i64,
    /// 佣金（单位：f64 × 1e6）
    pub commission: i64,
    /// 净数量（带方向符号）
    pub net_quantity: i64,
}

impl TradeRecord {
    /// 创建新交易记录
    pub fn new(trade: Trade, realized_pnl: i64, commission: i64, net_quantity: i64) -> Self {
        Self {
            trade,
            realized_pnl,
            commission,
            net_quantity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Timestamp;
    use crate::types::{Price, Quantity};

    #[test]
    fn test_trade_record_creation() {
        let trade = Trade::new(
            Timestamp::from_nanos(1_000),
            Price::from_f64(100.0),
            Quantity::from_f64(1.0),
            1,
            2,
        );
        let rec = TradeRecord::new(trade, 1_000_000, 100_000, 1_000_000);
        assert_eq!(rec.realized_pnl, 1_000_000);
        assert_eq!(rec.commission, 100_000);
    }
}
