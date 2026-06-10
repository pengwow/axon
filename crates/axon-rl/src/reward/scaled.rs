//! 奖励缩放器包装器

use serde::{Deserialize, Serialize};

use crate::action::state::PortfolioState;
use crate::action::types::Action;
use crate::reward::RewardFn;
use crate::reward::error::RewardError;

/// 奖励缩放器：包装任意 `RewardFn`，应用缩放和裁剪
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaledReward<R: RewardFn> {
    /// 内部奖励函数
    pub inner: R,
    /// 缩放因子
    pub scale: f64,
    /// 最小奖励值（裁剪下界）
    pub min_value: f64,
    /// 最大奖励值（裁剪上界）
    pub max_value: f64,
}

impl<R: RewardFn> ScaledReward<R> {
    /// 构造缩放奖励
    pub fn new(inner: R, scale: f64, min_value: f64, max_value: f64) -> Self {
        Self {
            inner,
            scale,
            min_value,
            max_value,
        }
    }
}

impl<R: RewardFn + Clone> RewardFn for ScaledReward<R> {
    fn calculate(
        &self,
        state: &PortfolioState,
        action: &Action,
        next_state: &PortfolioState,
        history: &[f64],
    ) -> Result<f64, RewardError> {
        let raw = self.inner.calculate(state, action, next_state, history)?;
        let scaled = raw * self.scale;
        Ok(scaled.clamp(self.min_value, self.max_value))
    }

    fn name(&self) -> &str {
        self.inner.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::types::Action;
    use crate::reward::pnl::PnLReward;

    fn make_state(value: f64) -> PortfolioState {
        PortfolioState {
            portfolio_value: value,
            cash: value,
            last_price: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_scale_multiplies_inner() {
        let scaled = ScaledReward::new(PnLReward::default(), 0.1, -10.0, 10.0);
        let s = make_state(100.0);
        let s2 = make_state(200.0);
        let a = Action::discrete(0);
        let reward = scaled.calculate(&s, &a, &s2, &[]).unwrap();
        // 100 * 0.1 = 10
        assert!((reward - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_clip_enforced() {
        let scaled = ScaledReward::new(PnLReward::default(), 1.0, -1.0, 1.0);
        let s = make_state(100.0);
        let s2 = make_state(200.0);
        let a = Action::discrete(0);
        let reward = scaled.calculate(&s, &a, &s2, &[]).unwrap();
        assert!((reward - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_inner_error_propagates() {
        let scaled = ScaledReward::new(PnLReward::default(), 1.0, -10.0, 10.0);
        let s = make_state(0.0);
        let mut s2 = make_state(100.0);
        s2.portfolio_value = f64::NAN;
        let a = Action::discrete(0);
        let err = scaled.calculate(&s, &a, &s2, &[]).unwrap_err();
        assert!(matches!(err, RewardError::InvalidPortfolioValue(_)));
    }

    #[test]
    fn test_name_inherits() {
        let scaled = ScaledReward::new(PnLReward::default(), 1.0, -10.0, 10.0);
        assert_eq!(scaled.name(), "absolute_pnl");
    }
}
