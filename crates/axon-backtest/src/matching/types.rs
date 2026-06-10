//! 撮合引擎相关类型定义
//!
//! 复用 [`axon_core`] 已有的 `Order` / `Price` / `Quantity` / `Symbol` / `Side` /
//! `OrderType` / `OrderStatus` / `OrderId` / `Timestamp` 等基础类型，
//! 本模块定义撮合引擎专用的成交记录 [`MatchFill`]、撮合角色 [`TradeRole`]、
//! 撮合结果 [`SubmitResult`] 等类型。

use serde::{Deserialize, Serialize};

use axon_core::market::Side;
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};

/// 撮合成交记录（撮合引擎内部使用）
///
/// 包含 taker / maker 双方信息、成交价、成交量、成交时间戳。
/// 与 axon-core 的 [`axon_core::event::FillEvent`] 不同：
/// 该结构是撮合引擎的核心数据类型，独立于事件流，
/// 调用方负责决定如何转换为 [`axon_core::event::FillEvent`] 或 [`axon_core::market::Trade`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchFill {
    /// 成交 ID（单调递增）
    pub fill_id: u64,
    /// 主动方订单 ID
    pub taker_order_id: u64,
    /// 被动方订单 ID
    pub maker_order_id: u64,
    /// 成交价格
    pub price: Price,
    /// 成交数量
    pub quantity: Quantity,
    /// 主动方方向
    pub taker_side: Side,
    /// 成交时间戳
    pub timestamp: Timestamp,
}

impl MatchFill {
    /// 成交金额
    #[inline]
    pub fn turnover(&self) -> f64 {
        self.price.as_f64() * self.quantity.as_f64()
    }
}

/// 撮合角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TradeRole {
    /// 挂单方（提供流动性）
    Maker,
    /// 吃单方（消耗流动性）
    Taker,
}

/// 订单簿层（撮合深度聚合）
///
/// 与 [`axon_core::market::OrderBookLevel`] 概念类似但语义不同：
/// 撮合引擎的 OrderBookLevel 是实时的内部数据结构，包含活跃订单数量。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrderBookLevel {
    /// 价格
    pub price: Price,
    /// 该价格层总数量
    pub quantity: Quantity,
    /// 订单数量
    pub order_count: usize,
}

impl OrderBookLevel {
    /// 创建新层级
    pub fn new(price: Price, quantity: Quantity, order_count: usize) -> Self {
        Self {
            price,
            quantity,
            order_count,
        }
    }
}

/// 订单提交结果
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SubmitResult {
    /// 撮合产生的成交列表
    pub fills: Vec<MatchFill>,
    /// 订单是否已完全成交
    pub is_filled: bool,
    /// 订单是否部分成交
    pub is_partially_filled: bool,
    /// 剩余未成交量
    pub remaining_quantity: Quantity,
}

impl SubmitResult {
    /// 创建空结果（订单未产生成交）
    pub fn empty(remaining_quantity: Quantity) -> Self {
        Self {
            fills: Vec::new(),
            is_filled: false,
            is_partially_filled: false,
            remaining_quantity,
        }
    }

    /// 创建已完全成交的结果
    pub fn filled(fills: Vec<MatchFill>) -> Self {
        Self {
            fills,
            is_filled: true,
            is_partially_filled: false,
            remaining_quantity: Quantity::default(),
        }
    }

    /// 创建部分成交结果
    pub fn partial(fills: Vec<MatchFill>, remaining_quantity: Quantity) -> Self {
        Self {
            fills,
            is_filled: false,
            is_partially_filled: true,
            remaining_quantity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axon_core::types::Price;

    #[test]
    fn test_match_fill_turnover() {
        let fill = MatchFill {
            fill_id: 0,
            taker_order_id: 1,
            maker_order_id: 2,
            price: Price::from_f64(100.0),
            quantity: Quantity::from_f64(2.0),
            taker_side: Side::Buy,
            timestamp: Timestamp::from_nanos(0),
        };
        assert_eq!(fill.turnover(), 200.0);
    }

    #[test]
    fn test_order_book_level_creation() {
        let level = OrderBookLevel::new(Price::from_f64(100.0), Quantity::from_f64(10.0), 3);
        assert_eq!(level.price, Price::from_f64(100.0));
        assert_eq!(level.order_count, 3);
    }

    #[test]
    fn test_submit_result_empty() {
        let r = SubmitResult::empty(Quantity::from_f64(10.0));
        assert!(r.fills.is_empty());
        assert!(!r.is_filled);
        assert!(!r.is_partially_filled);
        assert_eq!(r.remaining_quantity, Quantity::from_f64(10.0));
    }

    #[test]
    fn test_submit_result_filled() {
        let r = SubmitResult::filled(Vec::new());
        assert!(r.is_filled);
        assert!(!r.is_partially_filled);
    }

    #[test]
    fn test_submit_result_partial() {
        let r = SubmitResult::partial(Vec::new(), Quantity::from_f64(3.0));
        assert!(!r.is_filled);
        assert!(r.is_partially_filled);
        assert_eq!(r.remaining_quantity, Quantity::from_f64(3.0));
    }
}
