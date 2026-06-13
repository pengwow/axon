//! 交易所费率表
//!
//! 包含 [`FeeTable`]（阶梯费率 + 折扣）和 [`VolumeTier`]（单个阶梯定义），
//! 以及 Binance / Coinbase / Kraken 三个常用交易所的默认费率表。

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::error::{FeeModelError, FeeModelResult};
use super::types::{ExchangeId, FeeType, VolumeTier};

/// 交易所费率表
///
/// 多个 [`VolumeTier`] 阶梯按 `min_volume` 从小到大排列；
/// 查询时根据 `volume_30d` 找到对应阶梯。可选的平台币/机构折扣对所有阶梯统一应用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeTable {
    /// 交易所
    pub exchange: ExchangeId,
    /// 费率阶梯（按 `min_volume` 从小到大排序）
    pub tiers: Vec<VolumeTier>,
    /// 平台币抵扣折扣（如 0.10 = 10% off）
    pub native_token_discount: Option<Decimal>,
    /// 机构折扣
    pub institutional_discount: Option<Decimal>,
}

impl FeeTable {
    /// 创建新费率表（初始无阶梯、无折扣）
    pub fn new(exchange: ExchangeId) -> Self {
        Self {
            exchange,
            tiers: Vec::new(),
            native_token_discount: None,
            institutional_discount: None,
        }
    }

    /// 添加费率阶梯（添加后按 `min_volume` 排序）
    pub fn add_tier(mut self, tier: VolumeTier) -> Self {
        self.tiers.push(tier);
        self.tiers.sort_by_key(|t| t.min_volume);
        self
    }

    /// 设置平台币折扣
    pub fn with_native_token_discount(mut self, discount: Decimal) -> Self {
        self.native_token_discount = Some(discount);
        self
    }

    /// 设置机构折扣
    pub fn with_institutional_discount(mut self, discount: Decimal) -> Self {
        self.institutional_discount = Some(discount);
        self
    }

    /// 根据 30 日成交量查找对应阶梯
    ///
    /// 从高到低查找第一个满足 `volume_30d >= min_volume` 的阶梯；
    /// 无阶梯配置或成交量低于最低档时返回 None。
    pub fn find_tier(&self, volume_30d: Decimal) -> Option<&VolumeTier> {
        self.tiers
            .iter()
            .rev()
            .find(|tier| volume_30d >= tier.min_volume)
    }

    /// 阶梯数量
    #[inline]
    pub fn tier_count(&self) -> usize {
        self.tiers.len()
    }

    /// 计算 Maker 费用（含折扣）
    pub fn maker_fee(&self, notional: Decimal, volume_30d: Decimal) -> FeeModelResult<Decimal> {
        let tier = self
            .find_tier(volume_30d)
            .ok_or_else(|| FeeModelError::NoTiersConfigured(self.exchange.to_string()))?;
        let fee = tier.maker_fee.calculate(notional);
        Ok(self.apply_discounts(fee))
    }

    /// 计算 Taker 费用（含折扣）
    pub fn taker_fee(&self, notional: Decimal, volume_30d: Decimal) -> FeeModelResult<Decimal> {
        let tier = self
            .find_tier(volume_30d)
            .ok_or_else(|| FeeModelError::NoTiersConfigured(self.exchange.to_string()))?;
        let fee = tier.taker_fee.calculate(notional);
        Ok(self.apply_discounts(fee))
    }

    /// 顺序应用平台币折扣与机构折扣
    fn apply_discounts(&self, mut fee: Decimal) -> Decimal {
        if let Some(discount) = self.native_token_discount {
            fee *= dec!(1) - discount;
        }
        if let Some(discount) = self.institutional_discount {
            fee *= dec!(1) - discount;
        }
        fee
    }
}

// ─── 默认费率表（按 2024 年公开规则） ───────────────────────────

impl FeeTable {
    /// Binance 公开费率（2024）
    ///
    /// 普通用户 Maker/Taker 0.10%，VIP1-VIP9 阶梯递减至 0.02%/0.04%。
    pub fn binance_default() -> Self {
        Self::new(ExchangeId::Binance)
            .add_tier(VolumeTier {
                min_volume: dec!(0),
                maker_fee: FeeType::Percentage(dec!(0.00100)),
                taker_fee: FeeType::Percentage(dec!(0.00100)),
                label: "Regular".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(1_000_000),
                maker_fee: FeeType::Percentage(dec!(0.00090)),
                taker_fee: FeeType::Percentage(dec!(0.00100)),
                label: "VIP 1".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(5_000_000),
                maker_fee: FeeType::Percentage(dec!(0.00080)),
                taker_fee: FeeType::Percentage(dec!(0.00100)),
                label: "VIP 2".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(10_000_000),
                maker_fee: FeeType::Percentage(dec!(0.00042)),
                taker_fee: FeeType::Percentage(dec!(0.00060)),
                label: "VIP 3".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(100_000_000),
                maker_fee: FeeType::Percentage(dec!(0.00020)),
                taker_fee: FeeType::Percentage(dec!(0.00040)),
                label: "VIP 9".into(),
            })
    }

    /// Coinbase Advanced Trade 费率
    pub fn coinbase_default() -> Self {
        Self::new(ExchangeId::CoinbasePro)
            .add_tier(VolumeTier {
                min_volume: dec!(0),
                maker_fee: FeeType::Percentage(dec!(0.0040)),
                taker_fee: FeeType::Percentage(dec!(0.0060)),
                label: "Tier 1".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(10_000),
                maker_fee: FeeType::Percentage(dec!(0.0025)),
                taker_fee: FeeType::Percentage(dec!(0.0040)),
                label: "Tier 2".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(100_000),
                maker_fee: FeeType::Percentage(dec!(0.0015)),
                taker_fee: FeeType::Percentage(dec!(0.0025)),
                label: "Tier 3".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(1_000_000),
                maker_fee: FeeType::Percentage(dec!(0.0010)),
                taker_fee: FeeType::Percentage(dec!(0.0020)),
                label: "Tier 4".into(),
            })
    }

    /// Kraken Pro 费率
    pub fn kraken_default() -> Self {
        Self::new(ExchangeId::Kraken)
            .add_tier(VolumeTier {
                min_volume: dec!(0),
                maker_fee: FeeType::Percentage(dec!(0.0016)),
                taker_fee: FeeType::Percentage(dec!(0.0026)),
                label: "Tier 1".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(50_000),
                maker_fee: FeeType::Percentage(dec!(0.0014)),
                taker_fee: FeeType::Percentage(dec!(0.0024)),
                label: "Tier 2".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(1_000_000),
                maker_fee: FeeType::Percentage(dec!(0.0012)),
                taker_fee: FeeType::Percentage(dec!(0.0022)),
                label: "Tier 4".into(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;

    #[test]
    fn test_binance_default_tier_count() {
        let t = FeeTable::binance_default();
        assert_eq!(t.exchange, ExchangeId::Binance);
        assert_eq!(t.tier_count(), 5);
    }

    #[test]
    fn test_binance_regular_maker_fee() {
        // 0.1% * 50,000 = 50
        let t = FeeTable::binance_default();
        let fee = t.maker_fee(dec!(50_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(50));
    }

    #[test]
    fn test_binance_vip3_maker_fee() {
        // VIP 3 (10M+) → maker 0.042%, 50,000 * 0.00042 = 21
        let t = FeeTable::binance_default();
        let fee = t.maker_fee(dec!(50_000), dec!(10_000_000)).expect("fee");
        assert_eq!(fee, dec!(21));
    }

    #[test]
    fn test_tier_ladder_discount() {
        // 5M → VIP 2, maker 0.08% × 1000 = 0.8
        let t = FeeTable::binance_default();
        let fee = t.maker_fee(dec!(1_000), dec!(5_000_000)).expect("fee");
        assert_eq!(fee, dec!(0.8));
    }

    #[test]
    fn test_volume_tier_threshold() {
        // 恰好 1M → VIP 1
        let t = FeeTable::binance_default();
        let fee = t.maker_fee(dec!(1_000), dec!(1_000_000)).expect("fee");
        // 0.09% × 1000 = 0.9
        assert_eq!(fee, dec!(0.9));
    }

    #[test]
    fn test_native_token_discount_applied() {
        let t = FeeTable::binance_default().with_native_token_discount(dec!(0.10));
        // 50,000 * 0.001 = 50, BNB 10% off → 45
        let fee = t.taker_fee(dec!(50_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(45));
    }

    #[test]
    fn test_institutional_discount_applied() {
        let t = FeeTable::binance_default().with_institutional_discount(dec!(0.20));
        // 50,000 * 0.001 = 50, 20% off → 40
        let fee = t.taker_fee(dec!(50_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(40));
    }

    #[test]
    fn test_both_discounts_compose() {
        // 10% then 20% off = 50 * 0.9 * 0.8 = 36
        let t = FeeTable::binance_default()
            .with_native_token_discount(dec!(0.10))
            .with_institutional_discount(dec!(0.20));
        let fee = t.taker_fee(dec!(50_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(36));
    }

    #[test]
    fn test_tiered_sort_keeps_ascending() {
        // 即使乱序添加，也应按 min_volume 升序
        let t = FeeTable::new(ExchangeId::Binance)
            .add_tier(VolumeTier {
                min_volume: dec!(100_000_000),
                maker_fee: FeeType::Percentage(dec!(0.00020)),
                taker_fee: FeeType::Percentage(dec!(0.00040)),
                label: "VIP 9".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(0),
                maker_fee: FeeType::Percentage(dec!(0.00100)),
                taker_fee: FeeType::Percentage(dec!(0.00100)),
                label: "Regular".into(),
            });
        assert_eq!(t.tiers[0].min_volume, dec!(0));
        assert_eq!(t.tiers[1].min_volume, dec!(100_000_000));
    }

    #[test]
    fn test_find_tier_returns_none_when_empty() {
        let t = FeeTable::new(ExchangeId::Binance);
        assert!(t.find_tier(dec!(0)).is_none());
    }

    #[test]
    fn test_calculate_fee_errors_when_no_tiers() {
        let t = FeeTable::new(ExchangeId::Binance);
        let result = t.maker_fee(dec!(1_000), dec!(0));
        assert!(matches!(result, Err(FeeModelError::NoTiersConfigured(_))));
    }

    #[test]
    fn test_coinbase_default_tier_count() {
        let t = FeeTable::coinbase_default();
        assert_eq!(t.exchange, ExchangeId::CoinbasePro);
        assert_eq!(t.tier_count(), 4);
    }

    #[test]
    fn test_kraken_default_tier_count() {
        let t = FeeTable::kraken_default();
        assert_eq!(t.exchange, ExchangeId::Kraken);
        assert_eq!(t.tier_count(), 3);
    }

    #[test]
    fn test_table_clone_and_eq() {
        let t = FeeTable::binance_default();
        let t2 = t.clone();
        assert_eq!(t.exchange, t2.exchange);
        assert_eq!(t.tier_count(), t2.tier_count());
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 零名义金额 maker_fee
    #[test]
    fn test_maker_fee_zero_notional() {
        let t = FeeTable::binance_default();
        let fee = t.maker_fee(dec!(0), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(0));
    }

    /// 极大名义金额
    #[test]
    fn test_maker_fee_extreme_notional() {
        let t = FeeTable::binance_default();
        let fee = t.maker_fee(dec!(1000000000000000), dec!(0)).expect("fee");
        // 1e15 × 0.001 = 1e12
        assert_eq!(fee, dec!(1000000000000));
    }

    /// 负名义金额（异常：做空）
    #[test]
    fn test_maker_fee_negative_notional() {
        let t = FeeTable::binance_default();
        let fee = t.maker_fee(dec!(-1_000), dec!(0)).expect("fee");
        // -1000 × 0.001 = -1
        assert_eq!(fee, dec!(-1));
    }

    /// 单阶梯费率表
    #[test]
    fn test_single_tier_table() {
        let t = FeeTable::new(ExchangeId::Binance).add_tier(VolumeTier {
            min_volume: dec!(0),
            maker_fee: FeeType::Percentage(dec!(0.002)),
            taker_fee: FeeType::Percentage(dec!(0.004)),
            label: "Only".into(),
        });
        assert_eq!(t.tier_count(), 1);
        let fee = t.maker_fee(dec!(1_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(2));
    }

    /// 空阶梯表（无任何 tier）
    #[test]
    fn test_empty_tiers_table() {
        let t = FeeTable::new(ExchangeId::Binance);
        assert_eq!(t.tier_count(), 0);
        let result = t.maker_fee(dec!(1_000), dec!(0));
        assert!(matches!(result, Err(FeeModelError::NoTiersConfigured(_))));
    }

    /// 极小成交量 + 极小费率
    #[test]
    fn test_tiny_volume_tiny_rate() {
        let t = FeeTable::new(ExchangeId::Binance).add_tier(VolumeTier {
            min_volume: dec!(0),
            maker_fee: FeeType::Percentage(dec!(0.0001)),
            taker_fee: FeeType::Percentage(dec!(0.0001)),
            label: "Low".into(),
        });
        let fee = t.taker_fee(dec!(0.01), dec!(0)).expect("fee");
        // 0.01 × 0.0001 = 0.000001
        assert_eq!(fee, dec!(0.000001));
    }

    /// Fixed 费率类型：恒定金额
    #[test]
    fn test_maker_fee_fixed_amount() {
        let t = FeeTable::new(ExchangeId::Binance).add_tier(VolumeTier {
            min_volume: dec!(0),
            maker_fee: FeeType::Fixed(dec!(2.5)),
            taker_fee: FeeType::Fixed(dec!(3.5)),
            label: "Flat".into(),
        });
        // 不管 notional 多大，固定 2.5
        assert_eq!(t.maker_fee(dec!(1), dec!(0)).expect("fee"), dec!(2.5));
        assert_eq!(
            t.maker_fee(dec!(1_000_000), dec!(0)).expect("fee"),
            dec!(2.5)
        );
    }

    /// 100% 折扣 ⇒ 零费用
    #[test]
    fn test_native_token_discount_full() {
        let t = FeeTable::binance_default().with_native_token_discount(dec!(1.0));
        // 1 - 1.0 = 0 ⇒ 零费用
        let fee = t.taker_fee(dec!(50_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(0));
    }

    /// 100% 机构折扣 ⇒ 零费用
    #[test]
    fn test_institutional_discount_full() {
        let t = FeeTable::binance_default().with_institutional_discount(dec!(1.0));
        let fee = t.taker_fee(dec!(50_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(0));
    }

    /// 折扣 > 100%（异常：负费用）
    #[test]
    fn test_discount_above_one_yields_negative() {
        let t = FeeTable::binance_default().with_native_token_discount(dec!(1.5));
        // 50 × (1 - 1.5) = -25
        let fee = t.taker_fee(dec!(50_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(-25));
    }

    /// 多个折扣叠加
    #[test]
    fn test_discount_chain() {
        // 10% + 20% + 30% off = 50 × 0.9 × 0.8 × 0.7 = 25.2
        let t = FeeTable::binance_default()
            .with_native_token_discount(dec!(0.10))
            .with_institutional_discount(dec!(0.20));
        // 10% + 20% off = 50 × 0.9 × 0.8 = 36
        let fee = t.taker_fee(dec!(50_000), dec!(0)).expect("fee");
        assert_eq!(fee, dec!(36));
    }

    /// find_tier 极小正成交量
    #[test]
    fn test_find_tier_epsilon_volume() {
        let t = FeeTable::binance_default();
        let tier = t.find_tier(dec!(0.0001)).expect("tier");
        // 0.0001 < 1M ⇒ Regular
        assert_eq!(tier.label, "Regular");
    }

    /// find_tier 极大成交量
    #[test]
    fn test_find_tier_extreme_volume() {
        let t = FeeTable::binance_default();
        let tier = t.find_tier(dec!(1000000000000000)).expect("tier");
        // 应命中最高档 VIP 9
        assert_eq!(tier.label, "VIP 9");
    }

    /// find_tier 负成交量
    #[test]
    fn test_find_tier_negative_volume() {
        let t = FeeTable::binance_default();
        // 负值 < 0 ⇒ 不满足任何 tier（最低档 min=0）
        // 实际上 -1000 >= 0 为 false，所以返回 None
        assert!(t.find_tier(dec!(-1_000)).is_none());
    }

    /// add_tier 乱序排序：多次添加应保持升序
    #[test]
    fn test_tier_sort_random_order() {
        let t = FeeTable::new(ExchangeId::Binance)
            .add_tier(VolumeTier {
                min_volume: dec!(10_000_000),
                maker_fee: FeeType::Percentage(dec!(0.001)),
                taker_fee: FeeType::Percentage(dec!(0.001)),
                label: "C".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(100_000),
                maker_fee: FeeType::Percentage(dec!(0.001)),
                taker_fee: FeeType::Percentage(dec!(0.001)),
                label: "B".into(),
            })
            .add_tier(VolumeTier {
                min_volume: dec!(1_000),
                maker_fee: FeeType::Percentage(dec!(0.001)),
                taker_fee: FeeType::Percentage(dec!(0.001)),
                label: "A".into(),
            });
        assert_eq!(t.tiers[0].label, "A");
        assert_eq!(t.tiers[1].label, "B");
        assert_eq!(t.tiers[2].label, "C");
    }

    /// 序列化往返
    #[test]
    fn test_table_serde_roundtrip() {
        let t = FeeTable::binance_default()
            .with_native_token_discount(dec!(0.10))
            .with_institutional_discount(dec!(0.05));
        let json = serde_json::to_string(&t).unwrap();
        let de: FeeTable = serde_json::from_str(&json).unwrap();
        assert_eq!(de.tier_count(), 5);
        assert_eq!(de.native_token_discount, Some(dec!(0.10)));
        assert_eq!(de.institutional_discount, Some(dec!(0.05)));
    }

    /// 默认 Coinbase 阶梯查询
    #[test]
    fn test_coinbase_tier_lookup() {
        let t = FeeTable::coinbase_default();
        // volume = 50_000 ⇒ Tier 2 (10K-100K)
        let tier = t.find_tier(dec!(50_000)).expect("tier");
        assert_eq!(tier.label, "Tier 2");
    }

    /// 默认 Kraken 阶梯查询
    #[test]
    fn test_kraken_tier_lookup() {
        let t = FeeTable::kraken_default();
        // volume = 500_000 ⇒ Tier 2 (50K-1M)
        let tier = t.find_tier(dec!(500_000)).expect("tier");
        assert_eq!(tier.label, "Tier 2");
    }

    /// 多阶梯边界：volume = 阈值-1 / 阈值 / 阈值+1
    #[test]
    fn test_tier_boundary_values() {
        let t = FeeTable::binance_default();
        // 999_999 < 1M ⇒ Regular
        assert_eq!(t.find_tier(dec!(999_999)).unwrap().label, "Regular");
        // 1_000_000 = 1M ⇒ VIP 1
        assert_eq!(t.find_tier(dec!(1_000_000)).unwrap().label, "VIP 1");
        // 1_000_001 > 1M ⇒ VIP 1
        assert_eq!(t.find_tier(dec!(1_000_001)).unwrap().label, "VIP 1");
    }

    // ─── 并发测试 ──────────────────────────────────────────

    /// FeeTable 是不可变数据（所有方法仅 &self）：
    /// Arc 共享 + 多线程并发 maker_fee / taker_fee 查询应保持一致性
    #[test]
    fn test_concurrent_fee_lookup() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 50;
        const PER_THREAD: usize = 1_000;

        let table = Arc::new(FeeTable::binance_default());

        let mut handles = Vec::with_capacity(N_THREADS);
        for thread_id in 0..N_THREADS {
            let t = Arc::clone(&table);
            handles.push(thread::spawn(move || {
                for j in 0..PER_THREAD {
                    // 不同 thread 查询不同 volume 段（运行时构造 Decimal）
                    let volume = Decimal::from_u64((thread_id * 10_000 + j * 10) as u64).unwrap();
                    let notional = dec!(1_000);
                    let maker = t.maker_fee(notional, volume).expect("maker");
                    let taker = t.taker_fee(notional, volume).expect("taker");
                    // 验证 fee 是非负的
                    assert!(maker >= dec!(0));
                    assert!(taker >= dec!(0));
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    /// 多线程并发 find_tier 查询：返回结果应一致
    #[test]
    fn test_concurrent_find_tier() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 100;
        // 预留 PER_THREAD 容量以备后续压测扩展(随每个线程执行次数可调)
        #[allow(dead_code)]
        const PER_THREAD: usize = 100;

        let table = Arc::new(FeeTable::binance_default());

        let mut handles = Vec::with_capacity(N_THREADS);
        for _ in 0..N_THREADS {
            let t = Arc::clone(&table);
            handles.push(thread::spawn(move || {
                for v in [
                    dec!(0),
                    dec!(500_000),
                    dec!(1_000_000),
                    dec!(5_000_000),
                    dec!(10_000_000),
                    dec!(100_000_000),
                ] {
                    let tier = t.find_tier(v).expect("tier");
                    assert!(!tier.label.is_empty());
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    /// 多线程同时构造 FeeTable + 查询：每个线程独立构造、查询、销毁
    /// （构造过程涉及排序 ⇒ 验证 Decimal 排序在多线程下不共享可变状态）
    #[test]
    fn test_concurrent_independent_tables() {
        use std::thread;

        const N_THREADS: usize = 50;
        const PER_THREAD: usize = 100;

        let mut handles = Vec::with_capacity(N_THREADS);
        for thread_id in 0..N_THREADS {
            handles.push(thread::spawn(move || {
                for _ in 0..PER_THREAD {
                    // 每个线程独立构造
                    let t = FeeTable::binance_default()
                        .with_native_token_discount(dec!(0.10))
                        .with_institutional_discount(dec!(0.05));
                    let volume = Decimal::from_u64((thread_id * 10_000) as u64).unwrap();
                    let fee = t.taker_fee(dec!(1_000), volume).expect("fee");
                    // 验证 0 < fee < notional
                    assert!(fee > dec!(0));
                    assert!(fee < dec!(1_000));
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    /// 静态断言：FeeTable 是 Send + Sync（不可变数据天然支持）
    #[test]
    fn test_fee_table_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FeeTable>();
    }

    /// 高并发 + 边界 volume：查询阶梯边界值不 panic
    #[test]
    fn test_concurrent_boundary_lookups() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 50;
        const PER_THREAD: usize = 200;

        let table = Arc::new(FeeTable::binance_default());
        let boundary_volumes = [
            dec!(0),             // 最低档
            dec!(999_999),       // VIP 1 - 1
            dec!(1_000_000),     // VIP 1
            dec!(4_999_999),     // VIP 2 - 1
            dec!(5_000_000),     // VIP 2
            dec!(9_999_999),     // VIP 3 - 1
            dec!(10_000_000),    // VIP 3
            dec!(99_999_999),    // VIP 9 - 1
            dec!(100_000_000),   // VIP 9
            dec!(1_000_000_000), // 高于最大档
        ];

        let mut handles = Vec::with_capacity(N_THREADS);
        for _ in 0..N_THREADS {
            let t = Arc::clone(&table);
            handles.push(thread::spawn(move || {
                for j in 0..PER_THREAD {
                    let v = boundary_volumes[j % boundary_volumes.len()];
                    let tier = t.find_tier(v);
                    assert!(tier.is_some(), "所有 volume 都应能匹配到某个阶梯");
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }
}
