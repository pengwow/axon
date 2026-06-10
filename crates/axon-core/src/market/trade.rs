//! 成交记录

use serde::{Deserialize, Serialize};

use crate::order::OrderId;
use crate::time::Timestamp;
use crate::types::{Price, Quantity};

/// 成交记录（Trade）
///
/// 40 字节固定大小：`#[repr(C)]` 保证布局（5 个 8 字节字段，无 padding）
/// 8 (timestamp) + 8 (price) + 8 (quantity) + 8 (buyer) + 8 (seller) = 40 bytes
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct Trade {
    /// 成交时间
    pub timestamp: Timestamp,
    /// 成交价
    pub price: Price,
    /// 成交量
    pub quantity: Quantity,
    /// 买方订单 ID
    pub buyer_order_id: OrderId,
    /// 卖方订单 ID
    pub seller_order_id: OrderId,
}

impl Trade {
    /// 创建新 Trade
    #[inline]
    pub const fn new(
        timestamp: Timestamp,
        price: Price,
        quantity: Quantity,
        buyer_order_id: OrderId,
        seller_order_id: OrderId,
    ) -> Self {
        Self {
            timestamp,
            price,
            quantity,
            buyer_order_id,
            seller_order_id,
        }
    }

    /// 成交金额
    #[inline]
    pub fn turnover(&self) -> f64 {
        self.price.as_f64() * self.quantity.as_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_creation() {
        let t = Trade::new(
            Timestamp::from_nanos(1_000),
            Price::from_f64(100.0),
            Quantity::from_f64(10.0),
            42,
            99,
        );
        assert_eq!(t.timestamp, Timestamp::from_nanos(1_000));
        assert_eq!(t.price, Price::from_f64(100.0));
        assert_eq!(t.quantity, Quantity::from_f64(10.0));
        assert_eq!(t.buyer_order_id, 42);
        assert_eq!(t.seller_order_id, 99);
    }

    #[test]
    fn test_trade_turnover() {
        let t = Trade::new(
            Timestamp::from_nanos(0),
            Price::from_f64(50.0),
            Quantity::from_f64(20.0),
            1,
            2,
        );
        assert!((t.turnover() - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trade_timestamp_ordering() {
        let t1 = Trade::new(
            Timestamp::from_nanos(1_000),
            Price::from_f64(100.0),
            Quantity::from_f64(10.0),
            1,
            2,
        );
        let t2 = Trade::new(
            Timestamp::from_nanos(2_000),
            Price::from_f64(100.0),
            Quantity::from_f64(10.0),
            1,
            2,
        );
        assert!(t1.timestamp < t2.timestamp);
    }

    #[test]
    fn test_trade_default() {
        let t = Trade::default();
        assert_eq!(t.timestamp, Timestamp::default());
        assert_eq!(t.price, Price::default());
        assert_eq!(t.buyer_order_id, 0);
    }

    #[test]
    fn test_trade_size_is_40_bytes() {
        use std::mem::size_of;
        // 5 个 8 字节字段 = 40 字节（无 padding）
        assert_eq!(size_of::<Trade>(), 40);
    }
}
