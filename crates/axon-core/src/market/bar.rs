//! K线（Bar）— OHLCV 聚合数据

use serde::{Deserialize, Serialize};

use super::tick::Tick;
use crate::time::Timestamp;
use crate::types::{Price, Quantity};

/// K线时间周期
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BarPeriod {
    /// 1 分钟
    OneMinute,
    /// 5 分钟
    FiveMinutes,
    /// 15 分钟
    FifteenMinutes,
    /// 30 分钟
    ThirtyMinutes,
    /// 1 小时
    OneHour,
    /// 4 小时
    FourHours,
    /// 1 天
    OneDay,
    /// 1 周
    OneWeek,
    /// 1 月
    OneMonth,
}

impl BarPeriod {
    /// 返回该周期对应的纳秒数
    #[inline]
    pub fn nanos(self) -> i64 {
        match self {
            Self::OneMinute => 60 * 1_000_000_000,
            Self::FiveMinutes => 5 * 60 * 1_000_000_000,
            Self::FifteenMinutes => 15 * 60 * 1_000_000_000,
            Self::ThirtyMinutes => 30 * 60 * 1_000_000_000,
            Self::OneHour => 60 * 60 * 1_000_000_000,
            Self::FourHours => 4 * 60 * 60 * 1_000_000_000,
            Self::OneDay => 24 * 60 * 60 * 1_000_000_000,
            Self::OneWeek => 7 * 24 * 60 * 60 * 1_000_000_000,
            Self::OneMonth => 30 * 24 * 60 * 60 * 1_000_000_000, // 近似
        }
    }
}

/// K线（Bar）— OHLCV 数据
///
/// 使用 `#[repr(C)]` 固定布局，6 个 `Copy` 字段共 48 字节
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct Bar {
    /// K线起始时间
    pub timestamp: Timestamp,
    /// 开盘价
    pub open: Price,
    /// 最高价
    pub high: Price,
    /// 最低价
    pub low: Price,
    /// 收盘价
    pub close: Price,
    /// 成交量
    pub volume: Quantity,
}

impl Bar {
    /// 从多个 Tick 聚合生成 Bar
    ///
    /// 返回 `None` 当输入 ticks 为空
    pub fn from_ticks(ticks: &[Tick], _period: BarPeriod) -> Option<Self> {
        let first = ticks.first()?;
        let mut high = first.price.as_f64();
        let mut low = first.price.as_f64();
        let mut volume = 0.0_f64;

        for tick in ticks {
            high = high.max(tick.price.as_f64());
            low = low.min(tick.price.as_f64());
            volume += tick.quantity.as_f64();
        }

        let close = ticks.last()?.price;

        let bar = Self {
            timestamp: first.timestamp,
            open: first.price,
            high: Price::from_f64(high),
            low: Price::from_f64(low),
            close,
            volume: Quantity::from_f64(volume),
        };

        debug_assert!(
            bar.validate_ohlc().is_ok(),
            "Bar OHLC 验证失败：open={}, high={}, low={}, close={}",
            bar.open.as_f64(),
            bar.high.as_f64(),
            bar.low.as_f64(),
            bar.close.as_f64()
        );

        Some(bar)
    }

    /// 验证 OHLC 一致性
    pub fn validate_ohlc(&self) -> Result<(), super::MarketDataError> {
        if self.high.as_f64() < self.low.as_f64() {
            return Err(super::MarketDataError::OhlcInconsistent {
                high: self.high,
                low: self.low,
            });
        }
        if self.open.as_f64() > self.high.as_f64() || self.close.as_f64() > self.high.as_f64() {
            return Err(super::MarketDataError::OhlcInconsistent {
                high: self.high,
                low: self.low,
            });
        }
        if self.open.as_f64() < self.low.as_f64() || self.close.as_f64() < self.low.as_f64() {
            return Err(super::MarketDataError::OhlcInconsistent {
                high: self.high,
                low: self.low,
            });
        }
        Ok(())
    }

    /// 典型价格 = (high + low + close) / 3
    #[inline]
    pub fn typical_price(&self) -> f64 {
        (self.high.as_f64() + self.low.as_f64() + self.close.as_f64()) / 3.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::Side;

    fn make_tick(nanos: i64, price: f64, qty: f64) -> Tick {
        Tick::new(
            Timestamp::from_nanos(nanos),
            Price::from_f64(price),
            Quantity::from_f64(qty),
            Side::Buy,
        )
    }

    #[test]
    fn test_bar_ohlc_consistency() {
        let bar = Bar {
            timestamp: Timestamp::from_nanos(0),
            open: Price::from_f64(100.0),
            high: Price::from_f64(110.0),
            low: Price::from_f64(95.0),
            close: Price::from_f64(105.0),
            volume: Quantity::from_f64(1000.0),
        };
        assert!(bar.validate_ohlc().is_ok());
    }

    #[test]
    fn test_bar_from_ticks() {
        let ticks = vec![
            make_tick(1_000, 100.0, 10.0),
            make_tick(2_000, 105.0, 20.0),
            make_tick(3_000, 102.0, 15.0),
        ];
        let bar = Bar::from_ticks(&ticks, BarPeriod::OneMinute).unwrap();
        assert_eq!(bar.open, Price::from_f64(100.0));
        assert_eq!(bar.high, Price::from_f64(105.0));
        assert_eq!(bar.low, Price::from_f64(100.0));
        assert_eq!(bar.close, Price::from_f64(102.0));
        assert!((bar.volume.as_f64() - 45.0).abs() < f64::EPSILON);
        assert!(bar.validate_ohlc().is_ok());
    }

    #[test]
    fn test_bar_from_empty_ticks_returns_none() {
        let result = Bar::from_ticks(&[], BarPeriod::OneMinute);
        assert!(result.is_none());
    }

    #[test]
    fn test_bar_invalid_ohlc_panics() {
        // 构造一个 high < low 的非法 Bar
        let result = std::panic::catch_unwind(|| {
            let _ = Bar {
                timestamp: Timestamp::from_nanos(0),
                open: Price::from_f64(100.0),
                high: Price::from_f64(95.0), // 非法：high < low
                low: Price::from_f64(95.0),
                close: Price::from_f64(100.0),
                volume: Quantity::from_f64(10.0),
            }
            .validate_ohlc();
        });
        // validate_ohlc 返回 Err 而非 panic；测试 Err 即可
        // 此处改为测试 validate_ohlc 返回错误
        let bar = Bar {
            timestamp: Timestamp::from_nanos(0),
            open: Price::from_f64(100.0),
            high: Price::from_f64(95.0),
            low: Price::from_f64(95.0),
            close: Price::from_f64(100.0),
            volume: Quantity::from_f64(10.0),
        };
        assert!(bar.validate_ohlc().is_err());
        // 上面 catch_unwind 仅用于演示不会 panic
        let _ = result;
    }

    #[test]
    fn test_bar_typical_price() {
        let bar = Bar {
            timestamp: Timestamp::from_nanos(0),
            open: Price::from_f64(100.0),
            high: Price::from_f64(120.0),
            low: Price::from_f64(90.0),
            close: Price::from_f64(105.0),
            volume: Quantity::from_f64(0.0),
        };
        // (120 + 90 + 105) / 3 = 105
        assert!((bar.typical_price() - 105.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bar_period_nanos() {
        assert_eq!(BarPeriod::OneMinute.nanos(), 60 * 1_000_000_000);
        assert_eq!(BarPeriod::OneHour.nanos(), 3_600 * 1_000_000_000);
        assert_eq!(BarPeriod::OneDay.nanos(), 86_400 * 1_000_000_000);
    }

    #[test]
    fn test_bar_size_is_48_bytes() {
        use std::mem::size_of;
        assert_eq!(size_of::<Bar>(), 48);
    }
}
