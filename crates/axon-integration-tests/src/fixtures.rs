//! 共享测试辅助：合成数据生成、fixture、辅助函数

use std::collections::HashMap;
use std::path::PathBuf;

use axon_hpo::config::StudyDirection;
use axon_hpo::pareto::ParetoPoint;
use axon_hpo::trial::{TrialResult, TrialState};
use axon_walk_forward::metrics::{ISMetrics, OOSMetrics};
use rand::{Rng, SeedableRng};

/// 固定随机种子的合成市场收益率生成器
pub struct SyntheticReturns {
    /// 收益率序列
    pub returns: Vec<f64>,
    /// 真实平均收益
    pub mean: f64,
    /// 真实波动率
    pub std: f64,
}

impl SyntheticReturns {
    /// 生成指定长度的正态分布收益率（带漂移）
    ///
    /// 模拟一个趋势 + 噪声的金融时间序列
    pub fn generate(n: usize, mean: f64, std: f64, seed: u64) -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut returns = Vec::with_capacity(n);
        for i in 0..n {
            // 加上一个缓慢变化的趋势（模拟动量）
            let trend = (i as f64 / n as f64 - 0.5) * 0.001;
            let r = trend + rng.gen_range(-std..std) + mean;
            returns.push(r);
        }
        Self { returns, mean, std }
    }

    /// 模拟策略的 OOS 指标
    ///
    /// 给定一组超参数，模拟策略在某段 OOS 数据上的表现
    pub fn simulate_strategy_oos(
        &self,
        start: usize,
        end: usize,
        _params: &[(String, f64)],
    ) -> OOSMetrics {
        let slice = &self.returns[start..end.min(self.returns.len())];
        let total_return: f64 = slice.iter().sum();
        let mean = if !slice.is_empty() {
            total_return / slice.len() as f64
        } else {
            0.0
        };
        let variance = if slice.len() > 1 {
            slice.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (slice.len() - 1) as f64
        } else {
            0.0
        };
        let std = variance.sqrt();
        let sharpe = if std > 1e-9 {
            mean / std * (252.0_f64).sqrt()
        } else {
            0.0
        };

        // 最大回撤
        let mut cum = 1.0;
        let mut peak = 1.0;
        let mut max_dd = 0.0;
        for r in slice {
            cum *= 1.0 + r;
            if cum > peak {
                peak = cum;
            }
            let dd = (peak - cum) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
        let max_drawdown = -max_dd; // 负数

        let win_rate =
            slice.iter().filter(|&&r| r > 0.0).count() as f64 / slice.len().max(1) as f64;

        OOSMetrics {
            total_return,
            sharpe_ratio: sharpe,
            max_drawdown,
            win_rate,
            profit_factor: 1.0 + sharpe.abs() * 0.1,
            calmar_ratio: if max_drawdown.abs() > 1e-9 {
                (mean * 252.0) / max_drawdown.abs()
            } else {
                0.0
            },
        }
    }

    /// 模拟策略的 IS 指标（通常略好于 OOS）
    pub fn simulate_strategy_is(
        &self,
        start: usize,
        end: usize,
        params: &[(String, f64)],
    ) -> ISMetrics {
        let metrics = self.simulate_strategy_oos(start, end, params);
        // IS 通常比 OOS 略好
        ISMetrics {
            total_return: metrics.total_return * 1.05,
            sharpe_ratio: metrics.sharpe_ratio * 1.1,
            max_drawdown: metrics.max_drawdown,
            win_rate: metrics.win_rate,
            profit_factor: metrics.profit_factor,
        }
    }
}

/// 合成 trial 结果
///
/// - `params`: 试验超参（key, value）
/// - `value`: 目标函数值（单目标场景使用）
/// - `state`: 试验状态
pub fn make_trial(
    trial_id: i32,
    params: Vec<(String, f64)>,
    value: f64,
    state: TrialState,
) -> TrialResult {
    let mut param_map: HashMap<String, serde_json::Value> = HashMap::new();
    for (k, v) in params {
        param_map.insert(k, serde_json::json!(v));
    }
    TrialResult::new(trial_id, param_map, vec![value])
        .with_state(state)
        .with_duration(100)
}

/// 合成 Pareto 点（多目标）
///
/// - `objectives`: 各目标值
/// - `params`: 超参（key, value）
pub fn make_pareto(objectives: Vec<f64>, params: Vec<(String, f64)>) -> ParetoPoint {
    let mut param_map: HashMap<String, serde_json::Value> = HashMap::new();
    for (k, v) in params {
        param_map.insert(k, serde_json::json!(v));
    }
    ParetoPoint {
        params: param_map,
        objectives,
        trial_id: 0,
    }
}

/// 便利函数：多目标方向的辅助常量
pub fn both_directions_minimize() -> Vec<StudyDirection> {
    vec![StudyDirection::Minimize, StudyDirection::Minimize]
}

/// 便利函数：多目标方向的辅助常量
pub fn both_directions_maximize() -> Vec<StudyDirection> {
    vec![StudyDirection::Maximize, StudyDirection::Maximize]
}

/// 创建临时目录
pub fn temp_dir(_label: &str) -> PathBuf {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().to_path_buf();
    // 注意：tempdir 在 drop 时清理，这里我们保持简单返回路径
    std::mem::forget(dir);
    path
}

/// 简单 2D 抛物面目标函数（最大值在 (0.5, 0.3)）
///
/// 模拟 RL/策略超参搜索的常见情况
pub fn parabolic_objective(params: &[(String, f64)]) -> f64 {
    let x = params
        .iter()
        .find(|(k, _)| k == "x")
        .map(|(_, v)| *v)
        .unwrap_or(0.0);
    let y = params
        .iter()
        .find(|(k, _)| k == "y")
        .map(|(_, v)| *v)
        .unwrap_or(0.0);
    // 负值因为 HPO 默认 minimize
    -((x - 0.5).powi(2) + (y - 0.3).powi(2))
}
