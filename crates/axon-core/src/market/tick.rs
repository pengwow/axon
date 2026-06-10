//! 逐笔成交（Tick）

use serde::{Deserialize, Serialize};

use super::side::Side;
use crate::time::Timestamp;
use crate::types::{Price, Quantity};

/// 逐笔成交（Tick）
///
/// 高频数据最小单元，使用 `#[repr(C)]` 固定内存布局：
/// 8 (timestamp) + 8 (price) + 8 (quantity) + 1 (side) + 3 (padding) + 4 (尾部对齐) = 32 bytes
///
/// 注：未使用 `packed` 布局以避免 E0793 未对齐引用错误（性能权衡）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct Tick {
    /// 成交时间
    pub timestamp: Timestamp,
    /// 成交价
    pub price: Price,
    /// 成交量
    pub quantity: Quantity,
    /// 主动成交方向
    pub side: Side,
    /// 显式对齐填充，保证 `Copy` 语义
    #[serde(skip)]
    _pad: [u8; 3],
}

impl Tick {
    /// 创建新的 Tick
    #[inline]
    pub const fn new(timestamp: Timestamp, price: Price, quantity: Quantity, side: Side) -> Self {
        Self {
            timestamp,
            price,
            quantity,
            side,
            _pad: [0; 3],
        }
    }

    /// 成交金额 = price × quantity
    #[inline]
    pub fn turnover(&self) -> f64 {
        self.price.as_f64() * self.quantity.as_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_creation() {
        let ts = Timestamp::from_nanos(1_000);
        let tick = Tick::new(
            ts,
            Price::from_f64(100.5),
            Quantity::from_f64(10.0),
            Side::Buy,
        );
        assert_eq!(tick.timestamp, ts);
        assert_eq!(tick.price, Price::from_f64(100.5));
        assert_eq!(tick.quantity, Quantity::from_f64(10.0));
        assert_eq!(tick.side, Side::Buy);
    }

    #[test]
    fn test_tick_price_quantity_validation() {
        // 价格为负应自动归零
        let tick = Tick::new(
            Timestamp::from_nanos(0),
            Price::from_f64(-5.0),
            Quantity::from_f64(10.0),
            Side::Sell,
        );
        assert!(tick.price.as_f64() >= 0.0);
        // 数量允许为负（Position 用负数表示空头持仓）
        let tick2 = Tick::new(
            Timestamp::from_nanos(0),
            Price::from_f64(100.0),
            Quantity::from_f64(-3.0),
            Side::Buy,
        );
        assert_eq!(tick2.quantity.as_f64(), -3.0);
    }

    #[test]
    fn test_tick_turnover() {
        let tick = Tick::new(
            Timestamp::from_nanos(0),
            Price::from_f64(100.0),
            Quantity::from_f64(10.0),
            Side::Buy,
        );
        assert!((tick.turnover() - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tick_default() {
        let t = Tick::default();
        assert_eq!(t.timestamp, Timestamp::default());
        assert_eq!(t.price, Price::default());
        assert_eq!(t.quantity, Quantity::default());
        assert_eq!(t.side, Side::Buy);
    }

    #[test]
    fn test_tick_size_is_32_bytes() {
        use std::mem::size_of;
        // 8 字节对齐下 Tick 为 32 字节（含 _pad 与尾部对齐）
        assert_eq!(size_of::<Tick>(), 32);
    }
}
