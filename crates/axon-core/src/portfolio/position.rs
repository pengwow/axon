//! 单资产持仓

use serde::{Deserialize, Serialize};

use crate::market::Side;
use crate::types::{Price, Quantity, Symbol};

/// 单资产持仓
///
/// 数量符号表示方向：正数=多头，负数=空头。
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// 标的代码
    pub symbol: Symbol,
    /// 持仓数量（正数=多头，负数=空头）
    pub quantity: Quantity,
    /// 加权平均成本
    pub avg_cost: Price,
    /// 最新市场价格（用于未实现盈亏计算）
    pub market_price: Option<Price>,
    /// 已实现盈亏累计（单位：f64 × 1e6）
    pub realized_pnl: i64,
    /// 持仓方向（数量为 0 时默认为 Buy）
    pub side: Side,
}

impl Position {
    /// 创建新持仓
    pub fn new(symbol: Symbol, quantity: Quantity, avg_cost: Price) -> Self {
        let side = if quantity.as_f64() >= 0.0 {
            Side::Buy
        } else {
            Side::Sell
        };
        Self {
            symbol,
            quantity,
            avg_cost,
            market_price: None,
            realized_pnl: 0,
            side,
        }
    }

    /// 持仓市值（quantity × market_price）
    pub fn market_value(&self) -> Option<i64> {
        let mp = self.market_price?.as_f64();
        let v = self.quantity.as_f64() * mp;
        Some((v * 1_000_000.0) as i64)
    }

    /// 未实现盈亏（单位：f64 × 1e6）
    pub fn unrealized_pnl(&self) -> i64 {
        let mp = match self.market_price {
            Some(p) => p.as_f64(),
            None => return 0,
        };
        let qty = self.quantity.as_f64();
        let cost = self.avg_cost.as_f64();
        ((qty * (mp - cost)) * 1_000_000.0) as i64
    }

    /// 成本基础
    #[inline]
    pub fn cost_basis(&self) -> i64 {
        (self.quantity.as_f64().abs() * self.avg_cost.as_f64() * 1_000_000.0) as i64
    }

    /// 是否为空仓
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.quantity.as_f64().abs() < f64::EPSILON
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_long_position() {
        let p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(1.0),
            Price::from_f64(50_000.0),
        );
        assert_eq!(p.side, Side::Buy);
        assert!(!p.is_empty());
    }

    #[test]
    fn test_new_short_position() {
        let p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(-1.0),
            Price::from_f64(50_000.0),
        );
        assert_eq!(p.side, Side::Sell);
    }

    #[test]
    fn test_zero_quantity_is_empty() {
        let p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(0.0),
            Price::from_f64(0.0),
        );
        assert!(p.is_empty());
    }

    #[test]
    fn test_market_value() {
        let mut p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(1.0),
            Price::from_f64(50_000.0),
        );
        p.market_price = Some(Price::from_f64(55_000.0));
        let mv = p.market_value().unwrap();
        // 1.0 * 55000.0 = 55000.0
        assert!((mv - 55_000_000_000).abs() < 1_000_000);
    }

    #[test]
    fn test_market_value_no_market_price() {
        let p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(1.0),
            Price::from_f64(50_000.0),
        );
        assert!(p.market_value().is_none());
    }

    #[test]
    fn test_unrealized_pnl_long() {
        let mut p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(1.0),
            Price::from_f64(50_000.0),
        );
        p.market_price = Some(Price::from_f64(55_000.0));
        let upnl = p.unrealized_pnl();
        // 1.0 * (55000 - 50000) = 5000
        assert!((upnl - 5_000_000_000).abs() < 1_000_000);
    }

    #[test]
    fn test_unrealized_pnl_short() {
        let mut p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(-1.0),
            Price::from_f64(50_000.0),
        );
        p.market_price = Some(Price::from_f64(45_000.0));
        // -1.0 * (45000 - 50000) = 5000 (空头盈利)
        let upnl = p.unrealized_pnl();
        assert!(upnl > 0);
    }

    #[test]
    fn test_unrealized_pnl_no_market_price() {
        let p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(1.0),
            Price::from_f64(50_000.0),
        );
        assert_eq!(p.unrealized_pnl(), 0);
    }

    #[test]
    fn test_cost_basis() {
        let p = Position::new(
            Symbol::from("BTC-USDT"),
            Quantity::from_f64(2.0),
            Price::from_f64(50_000.0),
        );
        // 2.0 * 50000.0 = 100000.0
        assert!((p.cost_basis() - 100_000_000_000).abs() < 1_000_000);
    }

    #[test]
    fn test_default_via_derive() {
        // `#[derive(Default)]` 自动为所有字段使用默认值
        let p = Position::default();
        assert_eq!(p.symbol, Symbol::default());
        assert_eq!(p.quantity, Quantity::default());
        assert_eq!(p.avg_cost, Price::default());
        assert!(p.market_price.is_none());
        assert_eq!(p.realized_pnl, 0);
        assert_eq!(p.side, Side::default());
    }
}
