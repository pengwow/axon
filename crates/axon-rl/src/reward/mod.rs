//! 奖励函数核心 trait
//!
//! 定义 `RewardFn` trait 和共享类型。

use crate::action::state::PortfolioState;
use crate::action::types::Action;

pub mod error;
pub mod history;
pub mod multi_objective;
pub mod pnl;
pub mod scaled;
pub mod sharpe;

pub use error::{RewardError, RewardResult};
pub use history::ReturnHistory;
pub use multi_objective::MultiObjectiveReward;
pub use pnl::PnLReward;
pub use scaled::ScaledReward;
pub use sharpe::{RiskAdjustedType, SharpeReward};

/// 计算奖励的核心 trait
///
/// 所有奖励函数必须实现此 trait。`calculate` 方法接收当前状态、
/// 执行的动作、下一个状态和历史回报序列，返回一个标量奖励值。
///
/// 约定：
/// - 奖励值应在合理范围内（建议 `[-10.0, 10.0]`）
/// - 奖励应与优化目标方向一致（越大越好）
/// - 计算必须是纯函数式的，不产生副作用
pub trait RewardFn: Send + Sync {
    /// 根据状态转移计算奖励
    ///
    /// # Arguments
    /// * `state` - 执行动作前的组合状态
    /// * `action` - 代理执行的动作
    /// * `next_state` - 执行动作后的组合状态
    /// * `history` - 历史回报序列（用于风险调整奖励）
    ///
    /// # Returns
    /// 标量奖励值，建议范围 `[-10.0, 10.0]`
    fn calculate(
        &self,
        state: &PortfolioState,
        action: &Action,
        next_state: &PortfolioState,
        history: &[f64],
    ) -> Result<f64, RewardError>;

    /// 返回奖励函数的名称（用于日志和调试）
    fn name(&self) -> &str;

    /// 重置内部状态（如果有的话）
    fn reset(&mut self) {}
}

// ──────────────────────────────────────────────
// 工厂函数
// ──────────────────────────────────────────────

/// 根据配置字符串创建奖励函数
pub fn create_reward_fn(config: &str) -> Result<Box<dyn RewardFn>, RewardError> {
    match config {
        "pnl" => Ok(Box::new(PnLReward::default())),
        "relative_pnl" => Ok(Box::new(PnLReward {
            relative: true,
            ..Default::default()
        })),
        "sharpe" => Ok(Box::new(SharpeReward::default())),
        "sortino" => Ok(Box::new(SharpeReward {
            reward_type: RiskAdjustedType::Sortino,
            ..Default::default()
        })),
        other => Err(RewardError::UnknownConfig(other.to_string())),
    }
}

/// 创建标准多目标奖励（70% PnL, 20% 风险, 10% 换手率）
pub fn default_multi_objective() -> MultiObjectiveReward {
    MultiObjectiveReward::new(0.7, 0.2, 0.1)
}

// ──────────────────────────────────────────────
// 辅助函数
// ──────────────────────────────────────────────

/// 从组合净值序列计算收益率
pub fn compute_returns(portfolio_values: &[f64]) -> Vec<f64> {
    portfolio_values
        .windows(2)
        .map(|w| {
            if w[0] == 0.0 {
                0.0
            } else {
                (w[1] - w[0]) / w[0]
            }
        })
        .collect()
}

/// 计算累计收益率
pub fn compute_cumulative_return(portfolio_values: &[f64]) -> f64 {
    if portfolio_values.len() < 2 {
        return 0.0;
    }
    let first = portfolio_values[0];
    let last = *portfolio_values.last().unwrap();
    if first == 0.0 {
        0.0
    } else {
        (last / first) - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_returns_basic() {
        let values = vec![100.0, 110.0, 99.0, 121.0];
        let returns = compute_returns(&values);
        assert_eq!(returns.len(), 3);
        assert!((returns[0] - 0.1).abs() < 1e-9);
        assert!((returns[1] - (-0.1)).abs() < 1e-9);
        assert!((returns[2] - (121.0 / 99.0 - 1.0)).abs() < 1e-9);
    }

    #[test]
    fn test_compute_returns_empty() {
        assert!(compute_returns(&[]).is_empty());
        assert!(compute_returns(&[100.0]).is_empty());
    }

    #[test]
    fn test_compute_returns_handles_zero() {
        // 起点为 0 时返回 0（避免除零）
        let values = vec![0.0, 100.0];
        let returns = compute_returns(&values);
        assert_eq!(returns, vec![0.0]);
    }

    #[test]
    fn test_compute_cumulative_return_positive() {
        let values = vec![100.0, 110.0, 121.0];
        assert!((compute_cumulative_return(&values) - 0.21).abs() < 1e-9);
    }

    #[test]
    fn test_compute_cumulative_return_negative() {
        let values = vec![100.0, 80.0];
        assert!((compute_cumulative_return(&values) - (-0.2)).abs() < 1e-9);
    }

    #[test]
    fn test_compute_cumulative_return_short() {
        assert_eq!(compute_cumulative_return(&[]), 0.0);
        assert_eq!(compute_cumulative_return(&[100.0]), 0.0);
    }

    #[test]
    fn test_create_reward_fn_known() {
        assert!(create_reward_fn("pnl").is_ok());
        assert!(create_reward_fn("relative_pnl").is_ok());
        assert!(create_reward_fn("sharpe").is_ok());
        assert!(create_reward_fn("sortino").is_ok());
    }

    #[test]
    fn test_create_reward_fn_unknown() {
        let result = create_reward_fn("nope");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err, RewardError::UnknownConfig("nope".to_string()));
    }

    #[test]
    fn test_default_multi_objective() {
        let m = default_multi_objective();
        let total = m.pnl_weight + m.risk_weight + m.turnover_weight;
        assert!((total - 1.0).abs() < 1e-9);
    }
}
