//! PnL 奖励实现
//!
//! 基于组合净值变化的最简单奖励信号。

use serde::{Deserialize, Serialize};

use crate::action::state::PortfolioState;
use crate::action::types::Action;
use crate::reward::RewardFn;
use crate::reward::error::RewardError;

/// 基于 PnL 的简单奖励
///
/// 直接使用 `portfolio_value` 的变化量作为奖励。
/// 可选启用相对收益率（相对于期初资金）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PnLReward {
    /// 是否使用相对收益率（PnL / initial_capital）
    pub relative: bool,
    /// 奖励缩放因子（防止梯度爆炸）
    pub scale: f64,
    /// 期初资金（仅 `relative = true` 时使用），默认 0 表示自动使用 `state.portfolio_value`
    pub initial_capital: f64,
}

impl Default for PnLReward {
    fn default() -> Self {
        Self {
            relative: false,
            scale: 1.0,
            initial_capital: 0.0,
        }
    }
}

impl RewardFn for PnLReward {
    fn calculate(
        &self,
        state: &PortfolioState,
        _action: &Action,
        next_state: &PortfolioState,
        _history: &[f64],
    ) -> Result<f64, RewardError> {
        let pnl = next_state.portfolio_value - state.portfolio_value;

        let reward = if self.relative {
            let base = if self.initial_capital > 0.0 {
                self.initial_capital
            } else if state.portfolio_value > 0.0 {
                state.portfolio_value
            } else {
                return Err(RewardError::DivisionByZero);
            };
            pnl / base
        } else {
            pnl
        };

        let scaled = reward * self.scale;

        if !scaled.is_finite() {
            return Err(RewardError::InvalidPortfolioValue(scaled));
        }

        Ok(scaled)
    }

    fn name(&self) -> &str {
        if self.relative {
            "relative_pnl"
        } else {
            "absolute_pnl"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::types::Action;

    fn make_state(value: f64) -> PortfolioState {
        PortfolioState {
            portfolio_value: value,
            cash: value,
            last_price: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_absolute_pnl_positive_for_profit() {
        let r = PnLReward::default();
        let s = make_state(100.0);
        let s2 = make_state(110.0);
        let action = Action::discrete(0);
        let reward = r.calculate(&s, &action, &s2, &[]).unwrap();
        assert!((reward - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_absolute_pnl_negative_for_loss() {
        let r = PnLReward::default();
        let s = make_state(100.0);
        let s2 = make_state(95.0);
        let action = Action::discrete(0);
        let reward = r.calculate(&s, &action, &s2, &[]).unwrap();
        assert!((reward - (-5.0)).abs() < 1e-9);
    }

    #[test]
    fn test_relative_pnl_uses_initial_capital() {
        let r = PnLReward {
            relative: true,
            initial_capital: 1000.0,
            ..Default::default()
        };
        let s = make_state(100.0);
        let s2 = make_state(110.0);
        let action = Action::discrete(0);
        let reward = r.calculate(&s, &action, &s2, &[]).unwrap();
        assert!((reward - 0.01).abs() < 1e-9);
    }

    #[test]
    fn test_scale_multiplier() {
        let r = PnLReward {
            scale: 0.1,
            ..Default::default()
        };
        let s = make_state(100.0);
        let s2 = make_state(150.0);
        let action = Action::discrete(0);
        let reward = r.calculate(&s, &action, &s2, &[]).unwrap();
        assert!((reward - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_nan_value_returns_error() {
        let r = PnLReward::default();
        let s = make_state(0.0);
        let mut s2 = make_state(100.0);
        s2.portfolio_value = f64::NAN;
        let action = Action::discrete(0);
        let err = r.calculate(&s, &action, &s2, &[]).unwrap_err();
        assert!(matches!(err, RewardError::InvalidPortfolioValue(_)));
    }

    #[test]
    fn test_name() {
        assert_eq!(PnLReward::default().name(), "absolute_pnl");
        assert_eq!(
            PnLReward {
                relative: true,
                ..Default::default()
            }
            .name(),
            "relative_pnl"
        );
    }
}
