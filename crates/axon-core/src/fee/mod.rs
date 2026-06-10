//! 费用模型
//!
//! 回测时模拟真实交易所手续费、佣金、资金费用。
//! 实现单档/阶梯费率、平台币/机构折扣、跨交易所分档、资金费用计算与累计。
//!
//! TDD 规范：[`axon-design/01-tdd/01-phase1-core/13-fee-models.md`](../../../../axon-design/01-tdd/01-phase1-core/13-fee-models.md)
//!
//! # 模块组织
//!
//! - [`role`]：交易角色（Maker / Taker）
//! - [`types`]：核心类型（`FeeType` / `FeeBreakdown` / `FeeRecord` / `VolumeTier` / `ExchangeId`）
//! - [`table`]：[`FeeTable`] 费率表（含默认 Binance / Coinbase / Kraken 表）
//! - [`model`]：[`FeeModel`] trait + [`TieredFeeModel`] 阶梯费率模型
//! - [`error`]：[`FeeModelError`] 错误类型

pub mod error;
pub mod model;
pub mod role;
pub mod table;
pub mod types;

pub use error::{FeeModelError, FeeModelResult};
pub use model::{FeeModel, FeePosition, FeeTrade, TieredFeeModel};
pub use role::TradeRole;
pub use table::FeeTable;
pub use types::{ExchangeId, FeeBreakdown, FeeRecord, FeeType, VolumeTier};
