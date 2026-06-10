//! 交易环境配置

use serde::{Deserialize, Serialize};

/// 交易环境配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvConfig {
    /// 初始资金
    pub initial_capital: f64,
    /// 交易成本比例（如 `0.001 = 10 bps`）
    pub transaction_cost: f64,
    /// 滑点比例
    pub slippage: f64,
    /// 最大持仓比例（`0.0 ~ 1.0`）
    pub max_position_ratio: f64,
    /// 最大 episode 步数
    pub max_steps: usize,
    /// 随机种子
    pub seed: Option<u64>,
    /// 标的代码（用于组合状态关联）
    pub symbol: String,
    /// 收益率历史窗口（用于风险调整奖励）
    pub return_window: usize,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self {
            initial_capital: 100_000.0,
            transaction_cost: 0.001,
            slippage: 0.0005,
            max_position_ratio: 1.0,
            max_steps: 1000,
            seed: None,
            symbol: "BTCUSDT".to_string(),
            return_window: 252,
        }
    }
}
