//! 订单簿

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::error::MarketDataError;
use super::side::Side;
use crate::time::Timestamp;
use crate::types::{Price, Quantity};

/// 订单簿单层（价格 + 数量）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderBookLevel {
    /// 价格
    pub price: Price,
    /// 数量
    pub quantity: Quantity,
}

impl OrderBookLevel {
    /// 创建订单簿单层
    #[inline]
    pub const fn new(price: Price, quantity: Quantity) -> Self {
        Self { price, quantity }
    }
}

/// 订单簿快照
///
/// `bids` 按价格**降序**排列（最高买价在前），
/// `asks` 按价格**升序**排列（最低卖价在前）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    /// 快照时间
    pub timestamp: Timestamp,
    /// 买盘（降序）
    pub bids: Vec<OrderBookLevel>,
    /// 卖盘（升序）
    pub asks: Vec<OrderBookLevel>,
}

impl OrderBookSnapshot {
    /// 创建空订单簿
    pub fn empty(timestamp: Timestamp) -> Self {
        Self {
            timestamp,
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }

    /// 最优买价（最高买价）
    #[inline]
    pub fn best_bid(&self) -> Option<&OrderBookLevel> {
        self.bids.first()
    }

    /// 最优卖价（最低卖价）
    #[inline]
    pub fn best_ask(&self) -> Option<&OrderBookLevel> {
        self.asks.first()
    }

    /// 中间价 = (best_bid + best_ask) / 2
    pub fn mid_price(&self) -> Option<Price> {
        let bid = self.best_bid()?.price.as_f64();
        let ask = self.best_ask()?.price.as_f64();
        Some(Price::from_f64((bid + ask) / 2.0))
    }

    /// 买卖价差 (spread)
    pub fn spread(&self) -> Option<f64> {
        let bid = self.best_bid()?.price.as_f64();
        let ask = self.best_ask()?.price.as_f64();
        Some(ask - bid)
    }

    /// 买卖价差比率 = spread / mid_price
    pub fn spread_ratio(&self) -> Option<f64> {
        let spread = self.spread()?;
        let mid = self.mid_price()?.as_f64();
        if mid == 0.0 {
            return None;
        }
        Some(spread / mid)
    }

    /// 深度：从最优价开始累计到指定价格级别的总数量
    pub fn depth(&self, side: Side, levels: usize) -> Quantity {
        let book = match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        };
        let total: f64 = book.iter().take(levels).map(|l| l.quantity.as_f64()).sum();
        Quantity::from_f64(total)
    }

    /// 从 L2 数据构建，自动排序 + 过滤零数量层
    pub fn from_l2(
        timestamp: Timestamp,
        mut bids: Vec<OrderBookLevel>,
        mut asks: Vec<OrderBookLevel>,
    ) -> Self {
        // 买盘：按价格降序
        bids.sort_by(|a, b| {
            b.price
                .as_f64()
                .partial_cmp(&a.price.as_f64())
                .unwrap_or(Ordering::Equal)
        });
        // 卖盘：按价格升序
        asks.sort_by(|a, b| {
            a.price
                .as_f64()
                .partial_cmp(&b.price.as_f64())
                .unwrap_or(Ordering::Equal)
        });
        // 过滤零数量层
        bids.retain(|l| l.quantity.as_f64() > 0.0);
        asks.retain(|l| l.quantity.as_f64() > 0.0);
        Self {
            timestamp,
            bids,
            asks,
        }
    }

    /// 验证订单簿排序正确性
    pub fn validate_sorting(&self) -> Result<(), MarketDataError> {
        for window in self.bids.windows(2) {
            if window[0].price.as_f64() < window[1].price.as_f64() {
                return Err(MarketDataError::OrderBookUnsorted {
                    bid_level: window[0],
                    ask_level: window[1],
                });
            }
        }
        for window in self.asks.windows(2) {
            if window[0].price.as_f64() > window[1].price.as_f64() {
                return Err(MarketDataError::OrderBookUnsorted {
                    bid_level: window[0],
                    ask_level: window[1],
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> Timestamp {
        Timestamp::from_nanos(1_000)
    }

    fn level(price: f64, qty: f64) -> OrderBookLevel {
        OrderBookLevel::new(Price::from_f64(price), Quantity::from_f64(qty))
    }

    #[test]
    fn test_orderbook_best_bid_ask() {
        let bids = vec![level(99.0, 100.0), level(98.0, 200.0)];
        let asks = vec![level(100.0, 150.0), level(101.0, 300.0)];
        let ob = OrderBookSnapshot::from_l2(ts(), bids, asks);
        assert_eq!(ob.best_bid().unwrap().price, Price::from_f64(99.0));
        assert_eq!(ob.best_ask().unwrap().price, Price::from_f64(100.0));
    }

    #[test]
    fn test_orderbook_mid_price() {
        let bids = vec![level(99.0, 100.0)];
        let asks = vec![level(101.0, 100.0)];
        let ob = OrderBookSnapshot::from_l2(ts(), bids, asks);
        assert_eq!(ob.mid_price().unwrap(), Price::from_f64(100.0));
    }

    #[test]
    fn test_orderbook_spread() {
        let bids = vec![level(99.0, 100.0)];
        let asks = vec![level(100.0, 100.0)];
        let ob = OrderBookSnapshot::from_l2(ts(), bids, asks);
        assert!((ob.spread().unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_orderbook_spread_ratio() {
        let bids = vec![level(99.0, 100.0)];
        let asks = vec![level(101.0, 100.0)];
        let ob = OrderBookSnapshot::from_l2(ts(), bids, asks);
        // spread=2, mid=100, ratio=0.02
        assert!((ob.spread_ratio().unwrap() - 0.02).abs() < 1e-9);
    }

    #[test]
    fn test_orderbook_spread_zero_mid_returns_none() {
        // 中间价为 0 时 spread_ratio 返回 None
        // 0 数量会被 from_l2 过滤，先手动构造
        let mut ob = OrderBookSnapshot::empty(ts());
        ob.bids = vec![level(0.0, 100.0)];
        ob.asks = vec![level(0.0, 100.0)];
        assert!(ob.spread_ratio().is_none());
    }

    #[test]
    fn test_orderbook_depth() {
        let bids = vec![level(99.0, 100.0), level(98.0, 200.0), level(97.0, 300.0)];
        let asks = vec![level(100.0, 150.0)];
        let ob = OrderBookSnapshot::from_l2(ts(), bids, asks);
        // 前 2 层买盘总量 = 100 + 200 = 300
        assert_eq!(ob.depth(Side::Buy, 2), Quantity::from_f64(300.0));
        // 前 1 层卖盘 = 150
        assert_eq!(ob.depth(Side::Sell, 1), Quantity::from_f64(150.0));
    }

    #[test]
    fn test_orderbook_from_l2_sorts_bids_desc() {
        let bids = vec![level(98.0, 100.0), level(99.0, 200.0), level(97.0, 300.0)];
        let asks = vec![level(101.0, 100.0), level(100.0, 200.0)];
        let ob = OrderBookSnapshot::from_l2(ts(), bids, asks);
        // bids 降序
        assert_eq!(ob.bids[0].price, Price::from_f64(99.0));
        assert_eq!(ob.bids[1].price, Price::from_f64(98.0));
        assert_eq!(ob.bids[2].price, Price::from_f64(97.0));
        // asks 升序
        assert_eq!(ob.asks[0].price, Price::from_f64(100.0));
        assert_eq!(ob.asks[1].price, Price::from_f64(101.0));
    }

    #[test]
    fn test_orderbook_from_l2_filters_zero_quantity() {
        let bids = vec![level(99.0, 0.0), level(98.0, 100.0)];
        let asks = vec![level(100.0, 50.0), level(101.0, 0.0)];
        let ob = OrderBookSnapshot::from_l2(ts(), bids, asks);
        assert_eq!(ob.bids.len(), 1);
        assert_eq!(ob.asks.len(), 1);
    }

    #[test]
    fn test_orderbook_empty() {
        let ob = OrderBookSnapshot::empty(ts());
        assert!(ob.best_bid().is_none());
        assert!(ob.best_ask().is_none());
        assert!(ob.mid_price().is_none());
        assert!(ob.spread().is_none());
        assert!(ob.spread_ratio().is_none());
    }

    #[test]
    fn test_orderbook_validate_sorting_ok() {
        let bids = vec![level(99.0, 100.0), level(98.0, 200.0)];
        let asks = vec![level(100.0, 150.0)];
        let ob = OrderBookSnapshot::from_l2(ts(), bids, asks);
        assert!(ob.validate_sorting().is_ok());
    }

    #[test]
    fn test_orderbook_validate_sorting_detects_unsorted() {
        let mut ob = OrderBookSnapshot::empty(ts());
        // 手动构造乱序 bids
        ob.bids = vec![level(98.0, 100.0), level(99.0, 200.0)]; // 升序但 bids 期望降序
        ob.asks = vec![level(100.0, 150.0)];
        assert!(ob.validate_sorting().is_err());
    }
}
