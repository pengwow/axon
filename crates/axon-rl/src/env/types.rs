//! 行情数据 K 线

use serde::{Deserialize, Serialize};

/// K 线（OHLCV）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarketBar {
    /// 时间戳（毫秒）
    pub timestamp: u64,
    /// 开盘价
    pub open: f64,
    /// 最高价
    pub high: f64,
    /// 最低价
    pub low: f64,
    /// 收盘价
    pub close: f64,
    /// 成交量
    pub volume: f64,
}

impl MarketBar {
    /// 构造新 K 线
    pub fn new(timestamp: u64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        }
    }

    /// 典型价格 `(high + low + close) / 3`
    pub fn typical_price(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }
}

/// 订单执行结果
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    /// 标的代码
    pub symbol: String,
    /// 买卖方向
    pub side: crate::action::converter::OrderSide,
    /// 成交数量
    pub quantity: f64,
    /// 成交价格（含滑点）
    pub price: f64,
    /// 是否成交
    pub filled: bool,
    /// 交易成本
    pub cost: f64,
}

/// 环境信息（对应 Gymnasium 的 `info` dict）
#[derive(Debug, Clone, PartialEq)]
pub struct EnvInfo {
    /// 当前组合市值
    pub portfolio_value: f64,
    /// episode 累计成交笔数
    pub trades_executed: usize,
    /// episode 累计交易成本
    pub transaction_costs: f64,
    /// 当前时间步
    pub current_step: usize,
    /// 是否已结束
    pub done: bool,
    /// 初始资金
    pub initial_capital: f64,
}
