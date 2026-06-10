//! 多目标奖励：PnL + 风险 + 换手率惩罚

use serde::{Deserialize, Serialize};

use crate::action::state::PortfolioState;
use crate::action::types::Action;
use crate::reward::RewardFn;
use crate::reward::error::RewardError;
use crate::reward::pnl::PnLReward;
use crate::reward::sharpe::SharpeReward;

/// 多目标奖励
///
/// 将 PnL + 风险调整 + 换手率惩罚加权组合为单一标量。
/// 三个权重在 `new` 时自动归一化到 `sum = 1.0`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiObjectiveReward {
    /// PnL 子奖励权重
    pub pnl_weight: f64,
    /// 风险子奖励权重
    pub risk_weight: f64,
    /// 换手率惩罚权重
    pub turnover_weight: f64,
    /// PnL 子奖励
    pub pnl_reward: PnLReward,
    /// 风险子奖励
    pub risk_reward: SharpeReward,
    /// 换手率惩罚系数（绝对值）
    pub turnover_penalty: f64,
    /// 整体缩放因子
    pub scale: f64,
    /// 奖励裁剪上限
    pub clip: f64,
}

impl MultiObjectiveReward {
    /// 创建多目标奖励（自动归一化权重）
    pub fn new(pnl_weight: f64, risk_weight: f64, turnover_weight: f64) -> Self {
        let total = pnl_weight + risk_weight + turnover_weight;
        let (pw, rw, tw) = if total > 0.0 {
            (
                pnl_weight / total,
                risk_weight / total,
                turnover_weight / total,
            )
        } else {
            (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)
        };
        Self {
            pnl_weight: pw,
            risk_weight: rw,
            turnover_weight: tw,
            pnl_reward: PnLReward::default(),
            risk_reward: SharpeReward::default(),
            turnover_penalty: 0.01,
            scale: 1.0,
            clip: 10.0,
        }
    }

    /// 计算换手率（仓位比例绝对变化）
    fn calculate_turnover(
        state: &PortfolioState,
        _action: &Action,
        next_state: &PortfolioState,
    ) -> f64 {
        let old = state.position_ratio();
        let new = next_state.position_ratio();
        (new - old).abs().min(1.0)
    }
}

impl Default for MultiObjectiveReward {
    fn default() -> Self {
        Self::new(0.7, 0.2, 0.1)
    }
}

impl RewardFn for MultiObjectiveReward {
    fn calculate(
        &self,
        state: &PortfolioState,
        action: &Action,
        next_state: &PortfolioState,
        history: &[f64],
    ) -> Result<f64, RewardError> {
        // 验证权重和（容差 1e-6）
        let weight_sum = self.pnl_weight + self.risk_weight + self.turnover_weight;
        if (weight_sum - 1.0).abs() > 1e-6 {
            return Err(RewardError::InvalidWeightSum(weight_sum));
        }

        // 1. PnL 子奖励
        let pnl_reward = self
            .pnl_reward
            .calculate(state, action, next_state, history)?;

        // 2. 风险调整子奖励
        let risk_reward = self
            .risk_reward
            .calculate(state, action, next_state, history)?;

        // 3. 换手率惩罚（负奖励）
        let turnover = Self::calculate_turnover(state, action, next_state);
        let turnover_reward = -turnover * self.turnover_penalty;

        // 加权组合
        let combined = self.pnl_weight * pnl_reward
            + self.risk_weight * risk_reward
            + self.turnover_weight * turnover_reward;

        let scaled = combined * self.scale;

        if !scaled.is_finite() {
            return Err(RewardError::InvalidPortfolioValue(scaled));
        }

        Ok(scaled.clamp(-self.clip, self.clip))
    }

    fn name(&self) -> &str {
        "multi_objective"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::types::Action;

    fn make_state(value: f64, position: f64) -> PortfolioState {
        let last_price = 1.0;
        PortfolioState {
            portfolio_value: value,
            cash: value - position * last_price,
            position,
            last_price,
            ..Default::default()
        }
    }

    #[test]
    fn test_new_normalizes_weights() {
        let m = MultiObjectiveReward::new(7.0, 2.0, 1.0);
        assert!((m.pnl_weight - 0.7).abs() < 1e-9);
        assert!((m.risk_weight - 0.2).abs() < 1e-9);
        assert!((m.turnover_weight - 0.1).abs() < 1e-9);
    }

    #[test]
    fn test_default_weights_sum_to_one() {
        let m = MultiObjectiveReward::default();
        let sum = m.pnl_weight + m.risk_weight + m.turnover_weight;
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_turnover_penalty_reduces_reward() {
        // 两个状态的 PnL 相同，但换手率不同
        let s = make_state(100.0, 0.0);
        let s_low_turnover = make_state(105.0, 0.1);
        let s_high_turnover = make_state(105.0, 0.8);

        let m = MultiObjectiveReward::default();
        let a = Action::discrete(0);
        let r_low = m.calculate(&s, &a, &s_low_turnover, &[]).unwrap();
        let r_high = m.calculate(&s, &a, &s_high_turnover, &[]).unwrap();

        // 高换手率应被惩罚，所以 r_low ≥ r_high
        assert!(r_low >= r_high);
    }

    #[test]
    fn test_combined_reward_in_range() {
        let m = MultiObjectiveReward::default();
        let s = make_state(100.0, 0.0);
        let s2 = make_state(120.0, 0.5);
        let a = Action::discrete(0);
        let reward = m.calculate(&s, &a, &s2, &[]).unwrap();
        assert!((-10.0..=10.0).contains(&reward));
    }

    #[test]
    fn test_balances_goals() {
        // 测试加权和：三个分量必须都参与
        let m = MultiObjectiveReward::new(0.5, 0.3, 0.2);
        let s = make_state(100.0, 0.0);
        let s2 = make_state(110.0, 0.4);
        let a = Action::discrete(0);
        let reward = m.calculate(&s, &a, &s2, &[]).unwrap();
        // 仅检查非极端值（换手率 + PnL）
        assert!(reward.is_finite());
    }

    #[test]
    fn test_name() {
        assert_eq!(MultiObjectiveReward::default().name(), "multi_objective");
    }
}
