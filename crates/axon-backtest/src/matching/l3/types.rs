//! L3 撮合引擎相关类型

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use axon_core::types::{Price, Quantity, Symbol};

use super::super::types::OrderBookLevel;
use super::auction::BatchMode;

/// 交易场所标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Venue {
    /// Binance
    Binance,
    /// Coinbase
    Coinbase,
    /// Kraken
    Kraken,
    /// Bybit
    Bybit,
    /// OKX
    Okx,
    /// 火币
    Huobi,
    /// 自定义场所（使用 u16 ID）
    Custom(u16),
}

impl Venue {
    /// 场所名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::Binance => "binance",
            Self::Coinbase => "coinbase",
            Self::Kraken => "kraken",
            Self::Bybit => "bybit",
            Self::Okx => "okx",
            Self::Huobi => "huobi",
            Self::Custom(_) => "custom",
        }
    }
}

impl std::fmt::Display for Venue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// 跨资产交易对
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossPair {
    /// 第一腿资产（如 BTC/USDT）
    pub leg1: Symbol,
    /// 第二腿资产（如 ETH/USDT）
    pub leg2: Symbol,
    /// 交换比率（leg1 / leg2）
    pub ratio: f64,
    /// 最大可执行数量
    pub max_quantity: Quantity,
}

impl CrossPair {
    /// 创建新交易对（自动验证 leg1 != leg2）
    pub fn new(leg1: Symbol, leg2: Symbol, ratio: f64, max_quantity: Quantity) -> Self {
        Self {
            leg1,
            leg2,
            ratio,
            max_quantity,
        }
    }
}

/// 价格级别
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceLevel {
    /// 价格
    pub price: Price,
    /// 数量
    pub quantity: Quantity,
    /// 订单数量
    pub order_count: usize,
}

impl PriceLevel {
    /// 从 `OrderBookLevel` 转换
    pub fn from_book_level(level: &OrderBookLevel) -> Self {
        Self {
            price: level.price,
            quantity: level.quantity,
            order_count: level.order_count,
        }
    }
}

/// 单资产 L2 快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Snapshot {
    /// 资产
    pub symbol: Symbol,
    /// 最优买价
    pub best_bid: Option<Price>,
    /// 最优卖价
    pub best_ask: Option<Price>,
    /// 买单深度
    pub bid_depth: Vec<PriceLevel>,
    /// 卖单深度
    pub ask_depth: Vec<PriceLevel>,
    /// 成交笔数
    pub trade_count: u64,
}

/// 多资产订单簿快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingEngineSnapshot {
    /// 各资产的 L2 引擎快照
    pub engines: HashMap<Symbol, L2Snapshot>,
    /// 跨资产交易对配置
    pub cross_pairs: Vec<CrossPair>,
    /// 批量撮合模式
    pub batch_mode: BatchMode,
    /// 快照时间戳（Unix 纳秒）
    pub timestamp_ns: u64,
}

impl MatchingEngineSnapshot {
    /// 创建空快照
    pub fn empty(batch_mode: BatchMode) -> Self {
        Self {
            engines: HashMap::new(),
            cross_pairs: Vec::new(),
            batch_mode,
            timestamp_ns: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_venue_name() {
        assert_eq!(Venue::Binance.name(), "binance");
        assert_eq!(Venue::Coinbase.name(), "coinbase");
        assert_eq!(Venue::Custom(42).name(), "custom");
    }

    #[test]
    fn test_venue_display() {
        assert_eq!(format!("{}", Venue::Binance), "binance");
        assert_eq!(format!("{}", Venue::Kraken), "kraken");
    }

    #[test]
    fn test_cross_pair_new() {
        let p = CrossPair::new(
            "BTC/USDT".into(),
            "ETH/USDT".into(),
            0.06,
            Quantity::from_f64(10.0),
        );
        assert_eq!(p.ratio, 0.06);
        assert_eq!(p.max_quantity, Quantity::from_f64(10.0));
    }

    #[test]
    fn test_batch_mode_default() {
        assert_eq!(BatchMode::default(), BatchMode::Continuous);
    }

    #[test]
    fn test_price_level_from_book_level() {
        let book = OrderBookLevel::new(Price::from_f64(100.0), Quantity::from_f64(5.0), 3);
        let pl = PriceLevel::from_book_level(&book);
        assert_eq!(pl.price, Price::from_f64(100.0));
        assert_eq!(pl.quantity, Quantity::from_f64(5.0));
        assert_eq!(pl.order_count, 3);
    }

    #[test]
    fn test_snapshot_empty() {
        let s = MatchingEngineSnapshot::empty(BatchMode::Continuous);
        assert!(s.engines.is_empty());
        assert!(s.cross_pairs.is_empty());
        assert_eq!(s.batch_mode, BatchMode::Continuous);
    }
}
