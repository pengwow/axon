//! 费用模型核心类型

use rust_decimal::Decimal;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 交易所标识
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ExchangeId {
    /// Binance
    Binance,
    /// Coinbase Advanced Trade（前 Coinbase Pro）
    CoinbasePro,
    /// Kraken
    Kraken,
    /// Bybit
    Bybit,
    /// OKX
    Okx,
    /// 自定义交易所（提供名称）
    ///
    /// 使用 `String` 而非 `&'static str` 是为了支持 `Deserialize`；
    /// `Default` / `Copy` 改用 [`String`].
    Custom(String),
}

impl ExchangeId {
    /// 交易所名称（用于日志与错误消息）
    pub fn name(&self) -> &str {
        match self {
            Self::Binance => "binance",
            Self::CoinbasePro => "coinbase_pro",
            Self::Kraken => "kraken",
            Self::Bybit => "bybit",
            Self::Okx => "okx",
            Self::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for ExchangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// 费率类型：固定金额或按比例
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "value")
)]
pub enum FeeType {
    /// 按交易金额的百分比收取（例如 `0.001` = 0.1%）
    Percentage(Decimal),
    /// 按每笔固定金额收取
    Fixed(Decimal),
}

impl FeeType {
    /// 零费率
    pub const fn zero() -> Self {
        Self::Fixed(Decimal::ZERO)
    }

    /// 计算给定名义金额的费用
    #[inline]
    pub fn calculate(&self, notional: Decimal) -> Decimal {
        match self {
            Self::Percentage(rate) => notional * *rate,
            Self::Fixed(amount) => *amount,
        }
    }
}

/// 交易费用明细
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeBreakdown {
    /// 手续费（按交易金额比例或固定金额）
    pub commission: Decimal,
    /// 佣金（券商/平台额外收取）
    pub brokerage: Decimal,
    /// 资金费用（永续合约持仓费用，正=支付，负=收取）
    pub funding: Decimal,
    /// 总费用
    pub total: Decimal,
}

impl FeeBreakdown {
    /// 零费用
    pub const fn zero() -> Self {
        Self {
            commission: Decimal::ZERO,
            brokerage: Decimal::ZERO,
            funding: Decimal::ZERO,
            total: Decimal::ZERO,
        }
    }

    /// 仅含手续费的明细
    pub fn from_commission(commission: Decimal) -> Self {
        Self {
            commission,
            brokerage: Decimal::ZERO,
            funding: Decimal::ZERO,
            total: commission,
        }
    }

    /// 累加另一笔费用
    pub fn add(&mut self, other: &FeeBreakdown) {
        self.commission += other.commission;
        self.brokerage += other.brokerage;
        self.funding += other.funding;
        self.total += other.total;
    }
}

impl std::ops::Add for FeeBreakdown {
    type Output = Self;
    fn add(self, mut other: Self) -> Self {
        other.commission += self.commission;
        other.brokerage += self.brokerage;
        other.funding += self.funding;
        other.total += self.total;
        other
    }
}

/// 单笔费用记录（用于累计报告）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeeRecord {
    /// 成交 ID
    pub trade_id: u64,
    /// 标的代码
    pub instrument_id: String,
    /// 交易角色
    pub role: super::role::TradeRole,
    /// 费用明细
    pub fee_breakdown: FeeBreakdown,
    /// 时间戳（Unix 纳秒）
    pub timestamp: i64,
}

/// 交易所费率阶梯
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTier {
    /// 该阶梯的 30 日成交量下限（USDT）
    pub min_volume: Decimal,
    /// Maker 费率
    pub maker_fee: FeeType,
    /// Taker 费率
    pub taker_fee: FeeType,
    /// 描述（如 "VIP 1"）
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_exchange_id_name() {
        assert_eq!(ExchangeId::Binance.name(), "binance");
        assert_eq!(ExchangeId::CoinbasePro.name(), "coinbase_pro");
        assert_eq!(ExchangeId::Custom("myex".to_string()).name(), "myex");
    }

    #[test]
    fn test_exchange_id_display() {
        assert_eq!(format!("{}", ExchangeId::Binance), "binance");
        assert_eq!(format!("{}", ExchangeId::Custom("abc".to_string())), "abc");
    }

    #[test]
    fn test_fee_type_percentage() {
        let f = FeeType::Percentage(dec!(0.001));
        // 50000 * 0.001 = 50
        assert_eq!(f.calculate(dec!(50_000)), dec!(50));
    }

    #[test]
    fn test_fee_type_fixed() {
        let f = FeeType::Fixed(dec!(5));
        // 固定金额 5，无论 notional
        assert_eq!(f.calculate(dec!(1_000)), dec!(5));
        assert_eq!(f.calculate(dec!(10_000_000)), dec!(5));
    }

    #[test]
    fn test_fee_type_zero() {
        let f = FeeType::zero();
        assert_eq!(f.calculate(dec!(1_000)), Decimal::ZERO);
    }

    #[test]
    fn test_fee_breakdown_default() {
        let b = FeeBreakdown::default();
        assert_eq!(b.commission, Decimal::ZERO);
        assert_eq!(b.brokerage, Decimal::ZERO);
        assert_eq!(b.funding, Decimal::ZERO);
        assert_eq!(b.total, Decimal::ZERO);
    }

    #[test]
    fn test_fee_breakdown_from_commission() {
        let b = FeeBreakdown::from_commission(dec!(10));
        assert_eq!(b.commission, dec!(10));
        assert_eq!(b.total, dec!(10));
    }

    #[test]
    fn test_fee_breakdown_add() {
        let mut a = FeeBreakdown::from_commission(dec!(10));
        let b = FeeBreakdown {
            commission: dec!(5),
            brokerage: dec!(2),
            funding: dec!(1),
            total: dec!(8),
        };
        a.add(&b);
        assert_eq!(a.commission, dec!(15));
        assert_eq!(a.brokerage, dec!(2));
        assert_eq!(a.funding, dec!(1));
        assert_eq!(a.total, dec!(18));
    }

    #[test]
    fn test_fee_breakdown_add_operator() {
        let a = FeeBreakdown::from_commission(dec!(10));
        let b = FeeBreakdown {
            commission: dec!(5),
            brokerage: dec!(2),
            funding: dec!(1),
            total: dec!(8),
        };
        let c = a + b;
        assert_eq!(c.total, dec!(18));
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// Custom exchange_id 空字符串
    #[test]
    fn test_custom_exchange_empty_string() {
        let e = ExchangeId::Custom(String::new());
        assert_eq!(e.name(), "");
        assert_eq!(format!("{e}"), "");
    }

    /// Custom exchange_id 极大字符串
    #[test]
    fn test_custom_exchange_long_string() {
        let long = "x".repeat(10_000);
        let e = ExchangeId::Custom(long.clone());
        assert_eq!(e.name(), long);
    }

    /// ExchangeId Hash 一致性
    #[test]
    fn test_exchange_id_hash_consistency() {
        use std::collections::HashSet;
        let mut s = HashSet::new();
        s.insert(ExchangeId::Binance);
        s.insert(ExchangeId::Binance);
        s.insert(ExchangeId::CoinbasePro);
        assert_eq!(s.len(), 2);
    }

    /// Percentage 费率极小（1e-9）
    #[test]
    fn test_fee_type_percentage_min_positive_rate() {
        let f = FeeType::Percentage(dec!(0.000000001));
        // 1e9 × 1e-9 = 1
        assert_eq!(f.calculate(dec!(1_000_000_000)), dec!(1));
    }

    /// Percentage 费率 100%（1.0）
    #[test]
    fn test_fee_type_percentage_one_hundred_percent() {
        let f = FeeType::Percentage(dec!(1));
        // 100% × notional = notional
        assert_eq!(f.calculate(dec!(1000)), dec!(1000));
    }

    /// Percentage 费率 0%
    #[test]
    fn test_fee_type_percentage_zero_rate() {
        let f = FeeType::Percentage(dec!(0));
        assert_eq!(f.calculate(dec!(1_000_000)), dec!(0));
    }

    /// Percentage 费率 > 100%（异常配置）
    #[test]
    fn test_fee_type_percentage_rate_above_one() {
        let f = FeeType::Percentage(dec!(1.5));
        // 1.5 × 100 = 150
        assert_eq!(f.calculate(dec!(100)), dec!(150));
    }

    /// Percentage 零名义金额 ⇒ 零费用
    #[test]
    fn test_fee_type_percentage_zero_notional() {
        let f = FeeType::Percentage(dec!(0.001));
        assert_eq!(f.calculate(dec!(0)), dec!(0));
    }

    /// Fixed 极小固定金额
    #[test]
    fn test_fee_type_fixed_min_positive() {
        let f = FeeType::Fixed(dec!(0.000000001));
        assert_eq!(f.calculate(dec!(1_000_000)), dec!(0.000000001));
    }

    /// Fixed 负固定金额（异常配置：返佣）
    #[test]
    fn test_fee_type_fixed_negative() {
        let f = FeeType::Fixed(dec!(-5));
        assert_eq!(f.calculate(dec!(1_000)), dec!(-5));
    }

    /// Percentage + 负名义金额（异常：做空）
    #[test]
    fn test_fee_type_percentage_negative_notional() {
        let f = FeeType::Percentage(dec!(0.001));
        // -1000 × 0.001 = -1（异常场景）
        assert_eq!(f.calculate(dec!(-1000)), dec!(-1));
    }

    /// FeeBreakdown::default 与 zero 等价
    #[test]
    fn test_fee_breakdown_default_eq_zero() {
        assert_eq!(FeeBreakdown::default(), FeeBreakdown::zero());
    }

    /// FeeBreakdown::from_commission 负值
    #[test]
    fn test_fee_breakdown_from_negative_commission() {
        let b = FeeBreakdown::from_commission(dec!(-10));
        assert_eq!(b.commission, dec!(-10));
        assert_eq!(b.total, dec!(-10));
    }

    /// FeeBreakdown add 多个累积
    #[test]
    fn test_fee_breakdown_add_chain() {
        let mut a = FeeBreakdown::zero();
        for i in 1..=100 {
            // i: i32 转换为 Decimal（使用整数宏）
            let b = FeeBreakdown::from_commission(Decimal::from(i));
            a.add(&b);
        }
        // 1+2+...+100 = 5050
        assert_eq!(a.commission, dec!(5050));
        assert_eq!(a.total, dec!(5050));
    }

    /// FeeBreakdown add 自反性
    #[test]
    fn test_fee_breakdown_add_self() {
        let a = FeeBreakdown::from_commission(dec!(10));
        let mut b = a;
        b.add(&a);
        assert_eq!(b.commission, dec!(20));
    }

    /// FeeRecord 序列化往返
    #[test]
    fn test_fee_record_serde_roundtrip() {
        let r = FeeRecord {
            trade_id: 42,
            instrument_id: "BTC-USDT".into(),
            role: crate::fee::TradeRole::Taker,
            fee_breakdown: FeeBreakdown::from_commission(dec!(50)),
            timestamp: 1_000_000_000,
        };
        let json = serde_json::to_string(&r).unwrap();
        let de: FeeRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(de.trade_id, 42);
        assert_eq!(de.instrument_id, "BTC-USDT");
        assert_eq!(de.timestamp, 1_000_000_000);
    }

    /// VolumeTier 序列化往返
    #[test]
    fn test_volume_tier_serde_roundtrip() {
        let t = VolumeTier {
            min_volume: dec!(10_000),
            maker_fee: FeeType::Percentage(dec!(0.0005)),
            taker_fee: FeeType::Percentage(dec!(0.0010)),
            label: "VIP 1".into(),
        };
        let json = serde_json::to_string(&t).unwrap();
        let de: VolumeTier = serde_json::from_str(&json).unwrap();
        assert_eq!(de.label, "VIP 1");
        assert_eq!(de.min_volume, dec!(10_000));
    }

    /// ExchangeId equality 检查
    #[test]
    fn test_exchange_id_equality() {
        assert_eq!(ExchangeId::Binance, ExchangeId::Binance);
        assert_ne!(ExchangeId::Binance, ExchangeId::CoinbasePro);
        assert_eq!(
            ExchangeId::Custom("x".to_string()),
            ExchangeId::Custom("x".to_string())
        );
        assert_ne!(
            ExchangeId::Custom("x".to_string()),
            ExchangeId::Custom("y".to_string())
        );
    }
}
