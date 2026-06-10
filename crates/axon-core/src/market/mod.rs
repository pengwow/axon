//! 市场数据模型
//!
//! 包括 Tick（逐笔成交）、Bar（K线）、OrderBook（订单簿）快照和 Trade（成交）记录。
//!
//! TDD 规范：[`axon-design/01-tdd/01-phase1-core/02-market-data.md`](../../../../axon-design/01-tdd/01-phase1-core/02-market-data.md)

pub mod bar;
pub mod error;
pub mod orderbook;
pub mod side;
pub mod tick;
pub mod trade;

pub use bar::{Bar, BarPeriod};
pub use error::{MarketDataError, MarketDataResult};
pub use orderbook::{OrderBookLevel, OrderBookSnapshot};
pub use side::Side;
pub use tick::Tick;
pub use trade::Trade;
