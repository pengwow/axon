//! 风险调整奖励：Sharpe / Sortino

use serde::{Deserialize, Serialize};

use crate::action::state::PortfolioState;
use crate::action::types::Action;
use crate::reward::RewardFn;
use crate::reward::error::RewardError;

/// 风险调整奖励类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskAdjustedType {
    /// 夏普比率
    Sharpe,
    /// 索提诺比率（仅下行风险）
    Sortino,
}

/// 基于滚动窗口的风险调整奖励
///
/// 使用滚动窗口内的收益率计算夏普比率或索提诺比率。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SharpeReward {
    /// 滚动窗口大小（步数）
    pub window: usize,
    /// 无风险利率（年化），默认 0.0
    pub risk_free_rate: f64,
    /// 奖励缩放因子
    pub scale: f64,
    /// 奖励类型：Sharpe 或 Sortino
    pub reward_type: RiskAdjustedType,
    /// 单步奖励裁剪上限（绝对值）
    pub clip: f64,
}

impl Default for SharpeReward {
    fn default() -> Self {
        Self {
            window: 20,
            risk_free_rate: 0.0,
            scale: 1.0,
            reward_type: RiskAdjustedType::Sharpe,
            clip: 10.0,
        }
    }
}

impl RewardFn for SharpeReward {
    fn calculate(
        &self,
        state: &PortfolioState,
        _action: &Action,
        next_state: &PortfolioState,
        history: &[f64],
    ) -> Result<f64, RewardError> {
        // 计算当前步的收益率，附加到历史
        let current_return = if state.portfolio_value != 0.0 {
            (next_state.portfolio_value - state.portfolio_value) / state.portfolio_value
        } else {
            0.0
        };

        if history.len() < 2 {
            // 数据不足，退化为简单 PnL
            let pnl = next_state.portfolio_value - state.portfolio_value;
            return Ok((pnl * self.scale).clamp(-self.clip, self.clip));
        }

        // 构造窗口收益率：历史 + 当前
        let mut window_returns: Vec<f64> = history.to_vec();
        window_returns.push(current_return);

        let window_returns: Vec<f64> = if window_returns.len() > self.window {
            window_returns[window_returns.len() - self.window..].to_vec()
        } else {
            window_returns
        };

        let n = window_returns.len() as f64;
        let mean = window_returns.iter().sum::<f64>() / n;

        // 转换为日频无风险利率（假设 252 交易日）
        let daily_risk_free = self.risk_free_rate / 252.0;

        let ratio = match self.reward_type {
            RiskAdjustedType::Sharpe => {
                let variance = window_returns
                    .iter()
                    .map(|r| (r - mean).powi(2))
                    .sum::<f64>()
                    / (n - 1.0).max(1.0);

                if variance <= 0.0 || !variance.is_finite() {
                    0.0
                } else {
                    let std_dev = variance.sqrt();
                    if std_dev == 0.0 {
                        0.0
                    } else {
                        (mean - daily_risk_free) / std_dev
                    }
                }
            }
            RiskAdjustedType::Sortino => {
                // Sortino 只使用下行偏差
                let downside: Vec<f64> = window_returns
                    .iter()
                    .filter_map(|&r| {
                        if r < daily_risk_free {
                            Some((r - daily_risk_free).powi(2))
                        } else {
                            None
                        }
                    })
                    .collect();

                if downside.is_empty() {
                    // 无下行风险，给一个固定奖励（避免过拟合任意大值）
                    return Ok(self.clip * self.scale);
                }

                let downside_variance = downside.iter().sum::<f64>() / n;
                if downside_variance <= 0.0 || !downside_variance.is_finite() {
                    0.0
                } else {
                    let downside_std = downside_variance.sqrt();
                    if downside_std == 0.0 {
                        0.0
                    } else {
                        (mean - daily_risk_free) / downside_std
                    }
                }
            }
        };

        // 年化
        let annualized = ratio * 252.0_f64.sqrt();
        let scaled = annualized * self.scale;

        if !scaled.is_finite() {
            return Err(RewardError::InvalidPortfolioValue(scaled));
        }

        Ok(scaled.clamp(-self.clip, self.clip))
    }

    fn name(&self) -> &str {
        match self.reward_type {
            RiskAdjustedType::Sharpe => "sharpe",
            RiskAdjustedType::Sortino => "sortino",
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
    fn test_sharpe_penalizes_volatility() {
        // 同样均值，但波动率大者 sharpe 较低
        let r = SharpeReward {
            window: 4,
            ..Default::default()
        };
        let s = make_state(100.0);

        // 低波动：[+1%, +1%, +1%, +1%]
        let s_low = make_state(100.0 * 1.01_f64.powi(4));
        let history_low = vec![0.01, 0.01, 0.01];
        let reward_low = r
            .calculate(&s, &Action::discrete(0), &s_low, &history_low)
            .unwrap();

        // 高波动：[-5%, +5%, -5%, +5%]
        let s_high = make_state(100.0);
        let history_high = vec![-0.05, 0.05, -0.05];
        let reward_high = r
            .calculate(&s, &Action::discrete(0), &s_high, &history_high)
            .unwrap();

        // 低波动奖励 ≥ 高波动奖励（均值相同）
        assert!(reward_low > reward_high);
    }

    #[test]
    fn test_sortino_only_penalizes_downside() {
        let r = SharpeReward {
            window: 5,
            reward_type: RiskAdjustedType::Sortino,
            ..Default::default()
        };
        let s = make_state(100.0);

        // 只有上行（无下行）→ 触发固定最大奖励路径
        let s2 = make_state(101.0);
        let history = vec![0.02, 0.03, 0.04, 0.05];
        let reward = r
            .calculate(&s, &Action::discrete(0), &s2, &history)
            .unwrap();
        // 当下行为空时返回 self.clip * self.scale = 10.0
        assert!((reward - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_falls_back_to_pnl_with_short_history() {
        let r = SharpeReward::default();
        let s = make_state(100.0);
        let s2 = make_state(110.0);
        // history < 2，触发 PnL 退化路径
        let reward = r.calculate(&s, &Action::discrete(0), &s2, &[]).unwrap();
        // 退化为 pnl * scale = 10
        assert!((reward - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_zero_variance_returns_zero() {
        let r = SharpeReward {
            window: 5,
            ..Default::default()
        };
        let s = make_state(100.0);
        let s2 = make_state(100.0);
        // 全零收益，方差为 0
        let history = vec![0.0, 0.0, 0.0, 0.0];
        let reward = r
            .calculate(&s, &Action::discrete(0), &s2, &history)
            .unwrap();
        assert_eq!(reward, 0.0);
    }

    #[test]
    fn test_clip_enforced() {
        let r = SharpeReward {
            window: 3,
            clip: 1.0,
            ..Default::default()
        };
        let s = make_state(100.0);
        let s2 = make_state(1000.0);
        let history = vec![0.5, 0.6];
        let reward = r
            .calculate(&s, &Action::discrete(0), &s2, &history)
            .unwrap();
        // 经过 clip 应不超过 1.0
        assert!(reward <= 1.0);
        assert!(reward >= -1.0);
    }

    #[test]
    fn test_name() {
        assert_eq!(
            SharpeReward {
                reward_type: RiskAdjustedType::Sharpe,
                ..Default::default()
            }
            .name(),
            "sharpe"
        );
        assert_eq!(
            SharpeReward {
                reward_type: RiskAdjustedType::Sortino,
                ..Default::default()
            }
            .name(),
            "sortino"
        );
    }

    #[test]
    fn test_risk_free_rate_adjusts() {
        // 用较大的无风险利率 + 较小的 scale，避免 clip 触发
        let r_no_rf = SharpeReward {
            window: 30,
            risk_free_rate: 0.0,
            scale: 0.01,
            clip: 10.0,
            ..Default::default()
        };
        let r_with_rf = SharpeReward {
            window: 30,
            risk_free_rate: 1.0, // 100% 年化，远高于回报率
            scale: 0.01,
            clip: 10.0,
            ..Default::default()
        };
        let s = make_state(100.0);
        let s2 = make_state(100.05);
        let history = vec![0.001, 0.001];
        let a = Action::discrete(0);
        let reward_no = r_no_rf.calculate(&s, &a, &s2, &history).unwrap();
        let reward_rf = r_with_rf.calculate(&s, &a, &s2, &history).unwrap();
        // 100% 年化无风险利率（约 0.4%/日）高于回报率，奖励应为负且更小
        assert!(reward_rf < reward_no);
    }
}
