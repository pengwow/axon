//! Bar 聚合模块
//!
//! 提供 Tick → Bar 的聚合能力，支持 9 种非 Tick 频率(Min1 ~ Month1)。

mod bar_dataset;

pub use bar_dataset::{BarDataset, bar_schema, bars_to_batches};

use crate::error::{DataError, DataResult};
use crate::types::Frequency;
use axon_core::market::{Bar, Tick};
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};

/// Bar 聚合器(独立函数，不挂在 DataSource trait 上)
pub struct BarAggregator;

impl BarAggregator {
    /// 将 Tick 按指定频率聚合为 `Vec<Bar>`
    ///
    /// 前提:Tick 必须按时间戳升序排列(所有 Source 保证此语义)。
    /// Frequency::Tick 直接返回 InvalidRequest 错误。
    /// 离线场景下，所有有 tick 的 bucket 都产生 bar。
    pub fn aggregate_ticks(
        ticks: impl Iterator<Item = Tick>,
        frequency: Frequency,
    ) -> DataResult<Vec<Bar>> {
        if !frequency.is_bar() {
            return Err(DataError::UnsupportedFrequency("Tick".into()));
        }

        let mut bars: Vec<Bar> = Vec::new();
        let mut current_bucket: Option<i64> = None;
        let mut open = 0.0_f64;
        let mut high = f64::MIN;
        let mut low = f64::MAX;
        let mut close = 0.0_f64;
        let mut volume = 0.0_f64;

        for tick in ticks {
            let bucket = bucket_start(tick.timestamp.nanos, frequency)?;
            match current_bucket {
                Some(b) if b == bucket => {
                    // 同一个 bucket，累加 OHLCV
                    high = high.max(tick.price.as_f64());
                    low = low.min(tick.price.as_f64());
                    close = tick.price.as_f64();
                    volume += tick.quantity.as_f64();
                }
                Some(_) => {
                    // bucket 变化，推入上一个 bar
                    bars.push(Bar {
                        timestamp: Timestamp::from_nanos(current_bucket.unwrap()),
                        open: Price::from_f64(open),
                        high: Price::from_f64(high),
                        low: Price::from_f64(low),
                        close: Price::from_f64(close),
                        volume: Quantity::from_f64(volume),
                    });
                    // 开始新 bucket
                    current_bucket = Some(bucket);
                    open = tick.price.as_f64();
                    high = tick.price.as_f64();
                    low = tick.price.as_f64();
                    close = tick.price.as_f64();
                    volume = tick.quantity.as_f64();
                }
                None => {
                    // 第一个 tick
                    current_bucket = Some(bucket);
                    open = tick.price.as_f64();
                    high = tick.price.as_f64();
                    low = tick.price.as_f64();
                    close = tick.price.as_f64();
                    volume = tick.quantity.as_f64();
                }
            }
        }
        // 推入最后一个 bucket(如果有)
        if let Some(bucket) = current_bucket {
            bars.push(Bar {
                timestamp: Timestamp::from_nanos(bucket),
                open: Price::from_f64(open),
                high: Price::from_f64(high),
                low: Price::from_f64(low),
                close: Price::from_f64(close),
                volume: Quantity::from_f64(volume),
            });
        }

        Ok(bars)
    }
}

/// 时间戳 → bar bucket 起始时间(纳秒整数除法取整)
///
/// 关键:纳秒整数除法取整，零浮点误差。
fn bucket_start(ts_nanos: i64, freq: Frequency) -> DataResult<i64> {
    let bucket_ns = match freq {
        Frequency::Min1 => 60_000_000_000_i64,
        Frequency::Min5 => 300_000_000_000,
        Frequency::Min15 => 900_000_000_000,
        Frequency::Min30 => 1_800_000_000_000,
        Frequency::Hour1 => 3_600_000_000_000,
        Frequency::Hour4 => 14_400_000_000_000,
        Frequency::Day1 => 86_400_000_000_000,
        Frequency::Week1 => 604_800_000_000_000,
        // 月不均匀，简化为 30 天
        Frequency::Month1 => 2_592_000_000_000_000,
        Frequency::Tick => return Err(DataError::UnsupportedFrequency("Tick".into())),
    };
    Ok((ts_nanos / bucket_ns) * bucket_ns)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(nanos: i64, price: f64) -> Tick {
        Tick::new(
            Timestamp::from_nanos(nanos),
            Price::from_f64(price),
            Quantity::from_f64(1.0),
            axon_core::market::Side::Buy,
        )
    }

    #[test]
    fn bucket_start_min1_alignment() {
        // 0 秒 → 0, 59 秒 → 0, 60 秒 → 60s
        assert_eq!(bucket_start(0, Frequency::Min1).unwrap(), 0);
        assert_eq!(bucket_start(59_999_999_999, Frequency::Min1).unwrap(), 0);
        assert_eq!(
            bucket_start(60_000_000_000, Frequency::Min1).unwrap(),
            60_000_000_000
        );
    }

    #[test]
    fn bucket_start_hour1_alignment() {
        let hour_ns = 3_600_000_000_000_i64;
        assert_eq!(bucket_start(0, Frequency::Hour1).unwrap(), 0);
        assert_eq!(bucket_start(hour_ns - 1, Frequency::Hour1).unwrap(), 0);
        assert_eq!(bucket_start(hour_ns, Frequency::Hour1).unwrap(), hour_ns);
    }

    #[test]
    fn bucket_start_day1_alignment() {
        let day_ns = 86_400_000_000_000_i64;
        assert_eq!(bucket_start(0, Frequency::Day1).unwrap(), 0);
        assert_eq!(bucket_start(day_ns, Frequency::Day1).unwrap(), day_ns);
    }

    #[test]
    fn bucket_start_tick_returns_error() {
        assert!(bucket_start(0, Frequency::Tick).is_err());
    }

    #[test]
    fn aggregate_perfect_alignment() {
        // 10 个 tick 都在同一分钟内 → 1 个 bar
        let ticks: Vec<Tick> = (0..10)
            .map(|i| make_tick(i * 1_000_000_000, 100.0 + i as f64))
            .collect();
        let bars = BarAggregator::aggregate_ticks(ticks.into_iter(), Frequency::Min1).unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].open, Price::from_f64(100.0));
        assert_eq!(bars[0].high, Price::from_f64(109.0));
        assert_eq!(bars[0].low, Price::from_f64(100.0));
        assert_eq!(bars[0].close, Price::from_f64(109.0));
    }

    #[test]
    fn aggregate_cross_bucket_boundary() {
        // 2 个 tick 分别在不同分钟 → 2 个 bar
        let ticks = vec![make_tick(0, 100.0), make_tick(60_000_000_000, 200.0)];
        let bars = BarAggregator::aggregate_ticks(ticks.into_iter(), Frequency::Min1).unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].close, Price::from_f64(100.0));
        assert_eq!(bars[1].open, Price::from_f64(200.0));
    }

    #[test]
    fn aggregate_empty_ticks() {
        let bars = BarAggregator::aggregate_ticks(vec![].into_iter(), Frequency::Min1).unwrap();
        assert!(bars.is_empty());
    }

    #[test]
    fn aggregate_all_buckets_with_ticks_produce_bars() {
        // 2 个 tick 在不同 bucket → 2 个 bar(离线场景下所有有 tick 的 bucket 都完整)
        let ticks = vec![make_tick(0, 100.0), make_tick(61_000_000_000, 200.0)];
        let bars = BarAggregator::aggregate_ticks(ticks.into_iter(), Frequency::Min1).unwrap();
        assert_eq!(bars.len(), 2);
    }

    #[test]
    fn aggregate_tick_frequency_returns_error() {
        let ticks = vec![make_tick(0, 100.0)];
        let result = BarAggregator::aggregate_ticks(ticks.into_iter(), Frequency::Tick);
        assert!(result.is_err());
    }
}
