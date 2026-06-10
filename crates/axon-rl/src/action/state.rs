//! 投资组合状态（动作推断与 mask 的输入）

/// 投资组合状态：动作转换器据此计算订单大小与 mask
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PortfolioState {
    /// 当前持仓数量（正数 = 多头，负数 = 空头，0 = 空仓）
    pub position: f64,
    /// 可用现金
    pub cash: f64,
    /// 总资产（含持仓市值 + 现金）
    pub portfolio_value: f64,
    /// 已用保证金
    pub margin_used: f64,
    /// 可用保证金
    pub margin_available: f64,
    /// 浮动盈亏
    pub unrealized_pnl: f64,
    /// 最新成交价
    pub last_price: f64,
}

impl PortfolioState {
    /// 当前仓位价值（持仓 × 价格）
    pub fn position_value(&self) -> f64 {
        self.position * self.last_price
    }

    /// 当前仓位比例（仓位价值 / 组合总值）
    pub fn position_ratio(&self) -> f64 {
        if self.portfolio_value > 0.0 {
            self.position_value() / self.portfolio_value
        } else {
            0.0
        }
    }

    /// 是否为空仓
    pub fn is_flat(&self) -> bool {
        self.position == 0.0
    }
}
