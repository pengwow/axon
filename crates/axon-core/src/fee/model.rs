//! 费用模型 trait 与阶梯费率实现

use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::error::{FeeModelError, FeeModelResult};
use super::role::TradeRole;
use super::table::FeeTable;
use super::types::{ExchangeId, FeeBreakdown, FeeRecord, VolumeTier};

/// 费用计算输入：用于跨模块的轻量视图
///
/// fee 模块不复用 `market::Trade` / `portfolio::Position` 以保持解耦；
/// 调用方通过 `From` / 显式构造进行转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeTrade {
    /// 成交 ID
    pub trade_id: u64,
    /// 成交价
    pub price: Decimal,
    /// 成交量
    pub quantity: Decimal,
}

impl FeeTrade {
    /// 创建新视图
    pub const fn new(trade_id: u64, price: Decimal, quantity: Decimal) -> Self {
        Self {
            trade_id,
            price,
            quantity,
        }
    }

    /// 名义金额
    #[inline]
    pub fn notional(&self) -> Decimal {
        self.price * self.quantity
    }
}

/// 资金费用计算输入
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeePosition {
    /// 持仓量（带方向符号：正=多，负=空）
    pub quantity: Decimal,
    /// 加权平均成本
    pub avg_entry_price: Decimal,
}

impl FeePosition {
    /// 创建新视图
    pub const fn new(quantity: Decimal, avg_entry_price: Decimal) -> Self {
        Self {
            quantity,
            avg_entry_price,
        }
    }

    /// 名义持仓
    #[inline]
    pub fn notional(&self) -> Decimal {
        self.quantity.abs() * self.avg_entry_price
    }
}

/// 费用模型 trait
pub trait FeeModel: Send + Sync {
    /// 计算单笔交易费用
    fn calculate_fee(
        &self,
        exchange: ExchangeId,
        trade: &FeeTrade,
        role: TradeRole,
    ) -> FeeModelResult<FeeBreakdown>;

    /// 计算资金费用
    fn calculate_funding(&self, position: &FeePosition, rate: Decimal) -> Decimal;

    /// 获取当前适用的费率阶梯
    fn get_tier(&self, exchange: ExchangeId, volume_30d: Decimal) -> FeeModelResult<&VolumeTier>;

    /// 累计费用报告
    fn accumulate(&self, fees: &[FeeRecord]) -> FeeBreakdown;
}

/// 阶梯费率模型：按交易所维护费率表与 30 日成交量
#[derive(Debug, Default)]
pub struct TieredFeeModel {
    /// 交易所 → 费率表
    fee_tables: HashMap<ExchangeId, FeeTable>,
    /// 交易所 → 30 日成交量（USDT）
    volumes: HashMap<ExchangeId, Decimal>,
    /// 累计费用记录
    accumulated_fees: Vec<FeeRecord>,
}

impl TieredFeeModel {
    /// 创建空模型
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册交易所费率表
    pub fn register_exchange(&mut self, table: FeeTable) {
        let exchange = table.exchange.clone();
        self.fee_tables.insert(exchange, table);
    }

    /// 更新交易所 30 日成交量
    pub fn update_volume(&mut self, exchange: ExchangeId, volume: Decimal) {
        self.volumes.insert(exchange, volume);
    }

    /// 当前已注册交易所数量
    pub fn exchange_count(&self) -> usize {
        self.fee_tables.len()
    }

    /// 累计费用记录条数
    pub fn record_count(&self) -> usize {
        self.accumulated_fees.len()
    }

    /// 添加一条费用记录（用于报告）
    pub fn record(&mut self, record: FeeRecord) {
        self.accumulated_fees.push(record);
    }

    /// 获取已注册的交易所列表
    pub fn exchanges(&self) -> impl Iterator<Item = ExchangeId> + '_ {
        self.fee_tables.keys().cloned()
    }
}

impl FeeModel for TieredFeeModel {
    fn calculate_fee(
        &self,
        exchange: ExchangeId,
        trade: &FeeTrade,
        role: TradeRole,
    ) -> FeeModelResult<FeeBreakdown> {
        let table = self
            .fee_tables
            .get(&exchange)
            .ok_or_else(|| FeeModelError::ExchangeNotRegistered(exchange.to_string()))?;
        let volume = self.volumes.get(&exchange).copied().unwrap_or(dec!(0));

        let notional = trade.notional();
        let commission = match role {
            TradeRole::Maker => table.maker_fee(notional, volume)?,
            TradeRole::Taker => table.taker_fee(notional, volume)?,
        };

        Ok(FeeBreakdown::from_commission(commission))
    }

    fn calculate_funding(&self, position: &FeePosition, rate: Decimal) -> Decimal {
        position.notional() * rate
    }

    fn get_tier(&self, exchange: ExchangeId, volume_30d: Decimal) -> FeeModelResult<&VolumeTier> {
        let table = self
            .fee_tables
            .get(&exchange)
            .ok_or_else(|| FeeModelError::ExchangeNotRegistered(exchange.to_string()))?;
        table
            .find_tier(volume_30d)
            .ok_or_else(|| FeeModelError::NoTiersConfigured(exchange.to_string()))
    }

    fn accumulate(&self, fees: &[FeeRecord]) -> FeeBreakdown {
        let mut total = FeeBreakdown::zero();
        for fee in fees {
            total.add(&fee.fee_breakdown);
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_fee_trade_notional() {
        let t = FeeTrade::new(1, dec!(50_000), dec!(0.1));
        assert_eq!(t.notional(), dec!(5_000));
    }

    #[test]
    fn test_fee_position_notional_uses_abs_quantity() {
        // 空头 quantity = -10, abs = 10
        let p = FeePosition::new(dec!(-10), dec!(3_500));
        assert_eq!(p.notional(), dec!(35_000));
    }

    #[test]
    fn test_tiered_model_default() {
        let m = TieredFeeModel::new();
        assert_eq!(m.exchange_count(), 0);
        assert_eq!(m.record_count(), 0);
    }

    #[test]
    fn test_register_exchange() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        m.register_exchange(FeeTable::coinbase_default());
        assert_eq!(m.exchange_count(), 2);
    }

    #[test]
    fn test_calculate_fee_taker_binance() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        m.update_volume(ExchangeId::Binance, dec!(0));
        let trade = FeeTrade::new(1, dec!(50_000), dec!(1));
        let fee = m
            .calculate_fee(ExchangeId::Binance, &trade, TradeRole::Taker)
            .expect("fee");
        // 0.1% × 50000 = 50
        assert_eq!(fee.total, dec!(50));
        assert_eq!(fee.commission, dec!(50));
    }

    #[test]
    fn test_calculate_fee_maker_lower_than_taker() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        m.update_volume(ExchangeId::Binance, dec!(10_000_000)); // VIP 3
        let trade = FeeTrade::new(1, dec!(50_000), dec!(1));
        let maker = m
            .calculate_fee(ExchangeId::Binance, &trade, TradeRole::Maker)
            .expect("maker");
        let taker = m
            .calculate_fee(ExchangeId::Binance, &trade, TradeRole::Taker)
            .expect("taker");
        // VIP 3: maker 0.042%, taker 0.060%
        assert!(maker.total < taker.total);
        assert_eq!(maker.total, dec!(21));
        assert_eq!(taker.total, dec!(30));
    }

    #[test]
    fn test_calculate_fee_unregistered_exchange() {
        let m = TieredFeeModel::new();
        let trade = FeeTrade::new(1, dec!(1), dec!(1));
        let result = m.calculate_fee(ExchangeId::Binance, &trade, TradeRole::Taker);
        assert!(matches!(
            result,
            Err(FeeModelError::ExchangeNotRegistered(_))
        ));
    }

    #[test]
    fn test_calculate_funding_long_positive_rate() {
        let m = TieredFeeModel::new();
        let pos = FeePosition::new(dec!(10), dec!(3_500));
        // 多头 + 正费率 = 支付
        let funding = m.calculate_funding(&pos, dec!(0.0001));
        // 3500 * 10 * 0.0001 = 3.5
        assert_eq!(funding, dec!(3.5));
    }

    #[test]
    fn test_calculate_funding_short_negative_rate() {
        let m = TieredFeeModel::new();
        let pos = FeePosition::new(dec!(-10), dec!(3_500));
        // 负费率：空头支付 / 多头收取
        let funding = m.calculate_funding(&pos, dec!(-0.0001));
        // |quantity| × price × |rate| = 10 × 3500 × 0.0001 = 3.5
        // 但费率符号对所有人反向：rate=-0.0001 表示空头收取，所以 result = -3.5
        assert_eq!(funding, dec!(-3.5));
    }

    #[test]
    fn test_calculate_funding_zero_rate() {
        let m = TieredFeeModel::new();
        let pos = FeePosition::new(dec!(10), dec!(3_500));
        assert_eq!(m.calculate_funding(&pos, dec!(0)), dec!(0));
    }

    #[test]
    fn test_get_tier_for_volume() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let tier = m
            .get_tier(ExchangeId::Binance, dec!(10_000_000))
            .expect("tier");
        assert_eq!(tier.label, "VIP 3");
    }

    #[test]
    fn test_get_tier_unregistered() {
        let m = TieredFeeModel::new();
        let result = m.get_tier(ExchangeId::Binance, dec!(0));
        assert!(matches!(
            result,
            Err(FeeModelError::ExchangeNotRegistered(_))
        ));
    }

    #[test]
    fn test_get_tier_no_tiers() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::new(ExchangeId::Binance));
        let result = m.get_tier(ExchangeId::Binance, dec!(0));
        assert!(matches!(result, Err(FeeModelError::NoTiersConfigured(_))));
    }

    #[test]
    fn test_accumulate_empty() {
        let m = TieredFeeModel::new();
        let total = m.accumulate(&[]);
        assert_eq!(total, FeeBreakdown::zero());
    }

    #[test]
    fn test_accumulate_multiple_records() {
        let m = TieredFeeModel::new();
        let r1 = FeeRecord {
            trade_id: 1,
            instrument_id: "BTC-USDT".into(),
            role: TradeRole::Taker,
            fee_breakdown: FeeBreakdown {
                commission: dec!(50),
                brokerage: dec!(0),
                funding: dec!(0),
                total: dec!(50),
            },
            timestamp: 1_000_000_000,
        };
        let r2 = FeeRecord {
            trade_id: 2,
            instrument_id: "ETH-USDT".into(),
            role: TradeRole::Maker,
            fee_breakdown: FeeBreakdown {
                commission: dec!(10),
                brokerage: dec!(0),
                funding: dec!(0),
                total: dec!(10),
            },
            timestamp: 1_000_000_001,
        };
        let total = m.accumulate(&[r1, r2]);
        assert_eq!(total.commission, dec!(60));
        assert_eq!(total.total, dec!(60));
    }

    #[test]
    fn test_record_appends() {
        let mut m = TieredFeeModel::new();
        m.record(FeeRecord {
            trade_id: 1,
            instrument_id: "BTC-USDT".into(),
            role: TradeRole::Taker,
            fee_breakdown: FeeBreakdown::from_commission(dec!(50)),
            timestamp: 0,
        });
        m.record(FeeRecord {
            trade_id: 2,
            instrument_id: "ETH-USDT".into(),
            role: TradeRole::Taker,
            fee_breakdown: FeeBreakdown::from_commission(dec!(20)),
            timestamp: 0,
        });
        assert_eq!(m.record_count(), 2);
    }

    #[test]
    fn test_exchanges_iterator() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        m.register_exchange(FeeTable::kraken_default());
        let names: Vec<_> = m.exchanges().map(|e| e.to_string()).collect();
        assert!(names.contains(&"binance".to_string()));
        assert!(names.contains(&"kraken".to_string()));
    }

    #[test]
    fn test_tiered_discount_applied_to_tier() {
        // VIP 3 volume 50M → VIP 5+ (使用 binance_default 的 max VIP 9)
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let tier = m
            .get_tier(ExchangeId::Binance, dec!(200_000_000))
            .expect("tier");
        // 200M >= 100M → VIP 9
        assert_eq!(tier.label, "VIP 9");
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 零价格交易：notional = 0 ⇒ 零费用
    #[test]
    fn test_zero_price_trade_no_fee() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let trade = FeeTrade::new(1, dec!(0), dec!(100));
        let fee = m
            .calculate_fee(ExchangeId::Binance, &trade, TradeRole::Taker)
            .expect("fee");
        assert_eq!(fee.total, dec!(0));
    }

    /// 零数量交易：notional = 0 ⇒ 零费用
    #[test]
    fn test_zero_quantity_trade_no_fee() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let trade = FeeTrade::new(1, dec!(50_000), dec!(0));
        let fee = m
            .calculate_fee(ExchangeId::Binance, &trade, TradeRole::Taker)
            .expect("fee");
        assert_eq!(fee.total, dec!(0));
    }

    /// 负价格交易（异常：可能由浮点转换引发）
    #[test]
    fn test_negative_price_trade_negative_fee() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let trade = FeeTrade::new(1, dec!(-100), dec!(1));
        let fee = m
            .calculate_fee(ExchangeId::Binance, &trade, TradeRole::Taker)
            .expect("fee");
        // -100 × 0.001 = -0.1
        assert_eq!(fee.total, dec!(-0.1));
    }

    /// 负数量交易
    #[test]
    fn test_negative_quantity_trade() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let trade = FeeTrade::new(1, dec!(100), dec!(-2));
        let fee = m
            .calculate_fee(ExchangeId::Binance, &trade, TradeRole::Taker)
            .expect("fee");
        // 100 × -2 = -200 × 0.001 = -0.2
        assert_eq!(fee.total, dec!(-0.2));
    }

    /// 极大名义金额交易
    #[test]
    fn test_extreme_notional_trade() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let trade = FeeTrade::new(1, dec!(1_000_000), dec!(1_000_000));
        // 1e12 × 0.001 = 1e9
        let fee = m
            .calculate_fee(ExchangeId::Binance, &trade, TradeRole::Taker)
            .expect("fee");
        assert_eq!(fee.total, dec!(1_000_000_000));
    }

    /// Custom exchange 名
    #[test]
    fn test_custom_exchange_not_registered() {
        let m = TieredFeeModel::new();
        let trade = FeeTrade::new(1, dec!(1), dec!(1));
        let result = m.calculate_fee(
            ExchangeId::Custom("myex".to_string()),
            &trade,
            TradeRole::Taker,
        );
        assert!(matches!(
            result,
            Err(FeeModelError::ExchangeNotRegistered(_))
        ));
    }

    /// 资金费用：零持仓
    #[test]
    fn test_funding_zero_position() {
        let m = TieredFeeModel::new();
        let pos = FeePosition::new(dec!(0), dec!(3_500));
        assert_eq!(m.calculate_funding(&pos, dec!(0.0001)), dec!(0));
    }

    /// 资金费用：零均价
    #[test]
    fn test_funding_zero_price() {
        let m = TieredFeeModel::new();
        let pos = FeePosition::new(dec!(10), dec!(0));
        assert_eq!(m.calculate_funding(&pos, dec!(0.0001)), dec!(0));
    }

    /// 资金费用：极端 rate（1.0 = 100%）
    #[test]
    fn test_funding_extreme_rate() {
        let m = TieredFeeModel::new();
        let pos = FeePosition::new(dec!(1), dec!(100));
        // |1| × 100 × 1.0 = 100
        assert_eq!(m.calculate_funding(&pos, dec!(1.0)), dec!(100));
    }

    /// 资金费用：极小 rate
    #[test]
    fn test_funding_epsilon_rate() {
        let m = TieredFeeModel::new();
        let pos = FeePosition::new(dec!(1), dec!(1));
        // 1 × 1 × 0.000000001 = 0.000000001
        assert_eq!(
            m.calculate_funding(&pos, dec!(0.000000001)),
            dec!(0.000000001)
        );
    }

    /// get_tier 零成交量 ⇒ 最低档
    #[test]
    fn test_get_tier_zero_volume_lowest() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let tier = m.get_tier(ExchangeId::Binance, dec!(0)).expect("tier");
        assert_eq!(tier.label, "Regular");
    }

    /// get_tier 极小正成交量
    #[test]
    fn test_get_tier_epsilon_volume() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let tier = m.get_tier(ExchangeId::Binance, dec!(0.0001)).expect("tier");
        // 0.0001 < 1M ⇒ Regular
        assert_eq!(tier.label, "Regular");
    }

    /// get_tier 极小正成交量，恰好 1M ⇒ VIP 1
    #[test]
    fn test_get_tier_exact_threshold() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        let tier = m
            .get_tier(ExchangeId::Binance, dec!(1_000_000))
            .expect("tier");
        assert_eq!(tier.label, "VIP 1");
    }

    /// accumulate 大量记录（1000 笔）
    #[test]
    fn test_accumulate_many_records() {
        let m = TieredFeeModel::new();
        let records: Vec<FeeRecord> = (0..1_000)
            .map(|i| FeeRecord {
                trade_id: i as u64,
                instrument_id: "BTC-USDT".into(),
                role: TradeRole::Taker,
                fee_breakdown: FeeBreakdown::from_commission(dec!(1)),
                timestamp: i as i64,
            })
            .collect();
        let total = m.accumulate(&records);
        assert_eq!(total.commission, dec!(1_000));
        assert_eq!(total.total, dec!(1_000));
    }

    /// update_volume 负值：实际 get_tier 不使用 volumes（使用参数）
    #[test]
    fn test_update_volume_negative_does_not_affect_get_tier() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        m.update_volume(ExchangeId::Binance, dec!(-1000));
        // get_tier 实际不读 volumes，使用入参 volume_30d
        // 负值 -1000 < 任何 min_volume ⇒ None ⇒ NoTiersConfigured 错误
        let result = m.get_tier(ExchangeId::Binance, dec!(-1000));
        assert!(matches!(result, Err(FeeModelError::NoTiersConfigured(_))));
    }

    /// register_exchange 多次注册同一交易所 ⇒ 后者覆盖
    #[test]
    fn test_register_exchange_override() {
        let mut m = TieredFeeModel::new();
        m.register_exchange(FeeTable::binance_default());
        m.register_exchange(FeeTable::binance_default());
        // 仍然只 1 个 exchange
        assert_eq!(m.exchange_count(), 1);
    }

    /// exchanges 迭代器在空时为空
    #[test]
    fn test_exchanges_empty() {
        let m = TieredFeeModel::new();
        assert_eq!(m.exchanges().count(), 0);
    }

    /// record 后 record_count 增长
    #[test]
    fn test_record_count_grows() {
        let mut m = TieredFeeModel::new();
        assert_eq!(m.record_count(), 0);
        for i in 0..50 {
            m.record(FeeRecord {
                trade_id: i,
                instrument_id: "X".into(),
                role: TradeRole::Maker,
                fee_breakdown: FeeBreakdown::zero(),
                timestamp: 0,
            });
        }
        assert_eq!(m.record_count(), 50);
    }

    /// FeeTrade notional 边界
    #[test]
    fn test_fee_trade_notional_zero() {
        let t = FeeTrade::new(1, dec!(0), dec!(0));
        assert_eq!(t.notional(), dec!(0));
    }

    /// FeeTrade notional 极端值
    #[test]
    fn test_fee_trade_notional_extreme() {
        let t = FeeTrade::new(1, dec!(1_000_000), dec!(1_000_000));
        // 1e12
        assert_eq!(t.notional(), dec!(1000000000000));
    }

    /// FeePosition notional 空头
    #[test]
    fn test_fee_position_short_notional() {
        let p = FeePosition::new(dec!(-100), dec!(50));
        // abs(-100) × 50 = 5000
        assert_eq!(p.notional(), dec!(5_000));
    }

    /// FeePosition notional 零
    #[test]
    fn test_fee_position_zero_notional() {
        let p = FeePosition::new(dec!(0), dec!(50_000));
        assert_eq!(p.notional(), dec!(0));
    }

    /// TieredFeeModel: Send + Sync
    #[test]
    fn test_tiered_fee_model_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TieredFeeModel>();
    }
}
