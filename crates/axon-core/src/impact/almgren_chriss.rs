//! Almgren-Chriss 最优执行模型
//!
//! 经典学术模型（Almgren & Chriss 2000），用于在指定时间窗口 T 内交易大额订单 Q 时，
//! 平衡**市场冲击成本**与**价格波动风险**之间的权衡。
//!
//! # 模型
//!
//! 给定参数：
//! - `Q`：总交易数量
//! - `T`：总执行期（时间单位）
//! - `N`：执行步数
//! - `σ`：价格波动率（每单位时间）
//! - `ε`：临时冲击参数（每单位交易率）
//! - `η`：永久冲击参数
//! - `γ`：风险厌恶系数
//!
//! # 最优策略
//!
//! 关键参数：
//! - `κ = sqrt(γσ² / (ε × η))` —— 风险与冲击的相对重要性
//!
//! 最优轨迹（风险厌恶有限期）：
//! - `x_k = Q × sinh(κ × (T - t_k)) / sinh(κ × T)`
//! - `t_k = k × τ`，其中 `τ = T / N`
//!
//! 等分基线（`κ → 0`）：
//! - `x_k = Q × (1 - k/N)`
//!
//! # 期望成本与方差
//!
//! - 期望成本：`E[C] = ε × Σ v_k² + η × Q²`（含冲击与永久偏移）
//! - 成本方差：`Var[C] = σ² × Σ x_k² × τ`（剩余头寸 × 时间加权）
//!
//! # 实现差额（Implementation Shortfall）
//!
//! - `IS = avg_fill_price - arrival_price` 衡量实际成交均价相对决策时点价格的偏离
//! - 期望 IS 与参数化冲击成正比

use serde::{Deserialize, Serialize};

/// Almgren-Chriss 最优执行模型
///
/// 在指定时间窗口内执行大额订单时，给出最优交易轨迹以平衡市场冲击与价格风险。
///
/// # 字段说明
///
/// - `sigma`：价格波动率（每单位时间，标准差）
/// - `epsilon`：临时冲击系数（每单位交易率的价格偏移）
/// - `eta`：永久冲击系数（每单位累计交易量的价格偏移）
/// - `gamma`：风险厌恶系数（典型值：\(10^{-6} \sim 10^{-4}\)）
/// - `arrival_price`：决策时点中间价
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlmgrenChrissModel {
    /// 价格波动率（每 sqrt(time) 的标准差）
    pub sigma: f64,
    /// 临时冲击系数
    pub epsilon: f64,
    /// 永久冲击系数
    pub eta: f64,
    /// 风险厌恶系数
    pub gamma: f64,
    /// 决策时点中间价（用于计算实现差额）
    pub arrival_price: f64,
}

impl AlmgrenChrissModel {
    /// 创建新模型
    ///
    /// # 参数
    ///
    /// - `sigma`：价格波动率（> 0）
    /// - `epsilon`：临时冲击系数（> 0）
    /// - `eta`：永久冲击系数（> 0）
    /// - `gamma`：风险厌恶系数（≥ 0）
    /// - `arrival_price`：决策时点中间价
    pub fn new(sigma: f64, epsilon: f64, eta: f64, gamma: f64, arrival_price: f64) -> Self {
        assert!(sigma >= 0.0, "波动率必须非负");
        assert!(epsilon >= 0.0, "临时冲击系数必须非负");
        assert!(eta >= 0.0, "永久冲击系数必须非负");
        assert!(gamma >= 0.0, "风险厌恶系数必须非负");
        Self {
            sigma,
            epsilon,
            eta,
            gamma,
            arrival_price,
        }
    }

    /// 计算 κ = sqrt(γσ² / (ε × η))
    ///
    /// κ 衡量风险与冲击的相对重要性：
    /// - `κ → 0`：纯最小化冲击 → 等分执行
    /// - `κ → ∞`：纯最小化风险 → 立即执行
    pub fn kappa(&self) -> f64 {
        if self.epsilon <= 0.0 || self.eta <= 0.0 {
            return 0.0;
        }
        let numerator = self.gamma * self.sigma.powi(2);
        let denominator = self.epsilon * self.eta;
        if denominator <= 0.0 {
            return 0.0;
        }
        (numerator / denominator).sqrt()
    }

    /// 计算最优执行轨迹
    ///
    /// 返回长度为 `N` 的向量，每个元素为对应时间步**应交易**的数量（不是剩余头寸）。
    ///
    /// # 算法
    ///
    /// 论文中的最优交易率（连续形式）：
    /// `v*(t) = κ × Q × cosh(κ(T-t)) / sinh(κT)`
    ///
    /// 离散化（精确区间积分 + 归一化保证 `Σ n_k = Q`）：
    /// - `raw_k = ∫_{(k-1)τ}^{kτ} v*(t) dt = (Q / sinh(κT)) × [sinh(κ(T-(k-1)τ)) - sinh(κ(T-kτ))]`
    /// - `n_k = Q × raw_k / Σ raw_j`
    ///
    /// # 数值稳定性
    ///
    /// - 当 `κ × T < 1e-9` 时退化为等分执行
    /// - 当 `κ × T > 500`（sinh 溢出）时使用指数衰减近似
    pub fn optimal_trajectory(&self, total_quantity: f64, total_time: f64, n_steps: usize) -> Vec<f64> {
        assert!(total_quantity > 0.0, "总数量必须为正");
        assert!(total_time > 0.0, "总执行期必须为正");
        assert!(n_steps > 0, "执行步数必须为正");

        let kappa = self.kappa();
        let kappa_t = kappa * total_time;
        let tau = total_time / n_steps as f64;

        if kappa_t.abs() < 1e-9 {
            // κ × T → 0 ⇒ 等分执行
            let per_step = total_quantity / n_steps as f64;
            return vec![per_step; n_steps];
        }

        let mut raw = Vec::with_capacity(n_steps);
        if kappa_t > 500.0 {
            // 极小 κ（实为极大 κT）⇒ 指数衰减近似
            // 精确积分：(1/κ) × [exp(-κt_start) - exp(-κt_end)]
            for k in 0..n_steps {
                let t_start = k as f64 * tau;
                let t_end = (k + 1) as f64 * tau;
                let val = ((-kappa * t_start).exp() - (-kappa * t_end).exp()) / kappa;
                raw.push(val);
            }
        } else {
            // 精确区间积分：sinh(T-t_start) - sinh(T-t_end)
            for k in 0..n_steps {
                let t_start = k as f64 * tau;
                let t_end = (k + 1) as f64 * tau;
                let val = (kappa * (total_time - t_start)).sinh()
                    - (kappa * (total_time - t_end)).sinh();
                raw.push(val);
            }
        }

        // 归一化保证 Σ n_k = Q
        let raw_sum: f64 = raw.iter().sum();
        if raw_sum <= 0.0 || !raw_sum.is_finite() {
            // 极小 κT 退化：所有 raw 接近 0 ⇒ 全部归到 n_0（立即执行）
            let mut traj = vec![0.0; n_steps];
            traj[0] = total_quantity;
            return traj;
        }
        raw.into_iter()
            .map(|v| total_quantity * v / raw_sum)
            .collect()
    }

    /// 计算期望成本（含临时冲击 + 永久冲击）
    ///
    /// 公式（连续近似）：
    /// - 临时冲击：`ε × Σ (x_k² / τ)` —— `x_k/τ` 是交易率，平方后乘以 ε
    /// - 永久冲击：`η × Q²` —— 仅与总成交数量有关
    pub fn expected_cost(&self, trajectory: &[f64], total_time: f64) -> f64 {
        let n = trajectory.len();
        if n == 0 {
            return 0.0;
        }
        let tau = total_time / n as f64;
        let total_q: f64 = trajectory.iter().sum();
        // 临时冲击 = ε × Σ v_k² × τ = ε × Σ (x_k/τ)² × τ = ε × Σ x_k² / τ
        let temp_impact: f64 = trajectory.iter().map(|x| self.epsilon * x * x / tau).sum();
        let perm_impact = self.eta * total_q.powi(2);
        temp_impact + perm_impact
    }

    /// 计算成本方差（价格风险敞口）
    ///
    /// 公式：`Var[C] = σ² × Σ x_k² × τ`（剩余头寸 × 时间）
    ///
    /// 剩余头寸：`x_k` = 决策时总量 - 累计已成交
    pub fn cost_variance(&self, trajectory: &[f64], total_time: f64) -> f64 {
        let n = trajectory.len();
        if n == 0 {
            return 0.0;
        }
        let tau = total_time / n as f64;
        let total_q: f64 = trajectory.iter().sum();

        // 计算每步剩余头寸
        let mut remaining = total_q;
        let mut variance = 0.0;
        for &x_k in trajectory {
            // 使用 x_k 而非 remaining（更严格：每步暴露的剩余头寸）
            // 近似为：剩余头寸在区间 [remaining, remaining - x_k] 上的均值
            let exposed = remaining - x_k / 2.0;
            variance += self.sigma.powi(2) * exposed.powi(2) * tau;
            remaining -= x_k;
        }
        variance
    }

    /// 计算效用（E[C] + γ × Var[C]）
    ///
    /// 风险厌恶效用函数：成本期望 + 风险厌恶 × 成本方差
    /// 最小化此值即得到最优策略
    pub fn utility(&self, trajectory: &[f64], total_time: f64) -> f64 {
        let ec = self.expected_cost(trajectory, total_time);
        let var_c = self.cost_variance(trajectory, total_time);
        ec + self.gamma * var_c
    }

    /// 计算实现差额（Implementation Shortfall）
    ///
    /// 模拟在轨迹上按市价单成交后的加权平均成交价与 arrival price 之差。
    ///
    /// 假设临时冲击是交易率的线性函数：`ΔP_k = ε × v_k`
    pub fn implementation_shortfall(&self, trajectory: &[f64], total_time: f64) -> f64 {
        let n = trajectory.len();
        if n == 0 {
            return 0.0;
        }
        let tau = total_time / n as f64;
        let total_q: f64 = trajectory.iter().sum();
        if total_q.abs() < 1e-12 {
            return 0.0;
        }

        // 加权平均成交价 = arrival_price + 加权平均临时冲击
        let mut weighted_impact = 0.0;
        for &x_k in trajectory {
            let v_k = x_k / tau;
            let impact = self.epsilon * v_k;
            weighted_impact += impact * x_k;
        }
        let avg_impact = weighted_impact / total_q;

        avg_impact
    }
}

impl Default for AlmgrenChrissModel {
    /// 默认参数（中等波动、典型冲击、零风险厌恶）
    fn default() -> Self {
        Self {
            sigma: 0.02,        // 2% 波动率（每 sqrt(time)）
            epsilon: 0.01,      // 临时冲击系数
            eta: 0.001,         // 永久冲击系数
            gamma: 0.0,         // 无风险厌恶（等分执行）
            arrival_price: 100.0,
        }
    }
}

/// 单步执行信息
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// 步索引（0..N）
    pub step: usize,
    /// 该步应交易数量
    pub quantity: f64,
    /// 该步交易后累计剩余头寸
    pub remaining: f64,
    /// 该步成交价（相对 arrival）
    pub fill_price: f64,
}

/// 执行轨迹的便利结构
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// 步数
    pub n_steps: usize,
    /// 总时间窗口
    pub total_time: f64,
    /// 总数量
    pub total_quantity: f64,
    /// 步级详情
    pub steps: Vec<ExecutionStep>,
}

impl ExecutionPlan {
    /// 从轨迹构造执行计划
    pub fn from_trajectory(
        model: &AlmgrenChrissModel,
        trajectory: Vec<f64>,
        total_time: f64,
    ) -> Self {
        let total_quantity: f64 = trajectory.iter().sum();
        let tau = if !trajectory.is_empty() {
            total_time / trajectory.len() as f64
        } else {
            0.0
        };
        let mut remaining = total_quantity;
        let mut steps = Vec::with_capacity(trajectory.len());

        for (k, &q) in trajectory.iter().enumerate() {
            let v = if tau > 0.0 { q / tau } else { 0.0 };
            let impact = model.epsilon * v;
            let fill_price = model.arrival_price + impact;
            remaining -= q;
            steps.push(ExecutionStep {
                step: k,
                quantity: q,
                remaining,
                fill_price,
            });
        }

        Self {
            n_steps: trajectory.len(),
            total_time,
            total_quantity,
            steps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_valid_params() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 1e-6, 100.0);
        assert!((m.sigma - 0.02).abs() < 1e-10);
        assert!((m.epsilon - 0.01).abs() < 1e-10);
        assert!((m.eta - 0.001).abs() < 1e-10);
        assert!((m.gamma - 1e-6).abs() < 1e-10);
        assert!((m.arrival_price - 100.0).abs() < 1e-10);
    }

    #[test]
    #[should_panic(expected = "波动率必须非负")]
    fn test_new_rejects_negative_sigma() {
        AlmgrenChrissModel::new(-0.02, 0.01, 0.001, 0.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "临时冲击系数必须非负")]
    fn test_new_rejects_negative_epsilon() {
        AlmgrenChrissModel::new(0.02, -0.01, 0.001, 0.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "永久冲击系数必须非负")]
    fn test_new_rejects_negative_eta() {
        AlmgrenChrissModel::new(0.02, 0.01, -0.001, 0.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "风险厌恶系数必须非负")]
    fn test_new_rejects_negative_gamma() {
        AlmgrenChrissModel::new(0.02, 0.01, 0.001, -1e-6, 100.0);
    }

    #[test]
    fn test_default_model() {
        let m = AlmgrenChrissModel::default();
        assert!(m.sigma > 0.0);
        assert_eq!(m.arrival_price, 100.0);
    }

    #[test]
    fn test_kappa_zero_when_gamma_zero() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 0.0, 100.0);
        assert_eq!(m.kappa(), 0.0);
    }

    #[test]
    fn test_kappa_zero_when_epsilon_zero() {
        let m = AlmgrenChrissModel::new(0.02, 0.0, 0.001, 1e-6, 100.0);
        assert_eq!(m.kappa(), 0.0);
    }

    #[test]
    fn test_kappa_positive() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 1e-6, 100.0);
        // κ = sqrt(1e-6 × 4e-4 / (0.01 × 0.001)) = sqrt(4e-5) ≈ 0.00632
        let k = m.kappa();
        assert!(k > 0.0);
        assert!((k - 0.00632).abs() < 1e-4);
    }

    #[test]
    fn test_optimal_trajectory_zero_gamma_is_uniform() {
        // γ = 0 ⇒ κ = 0 ⇒ 等分执行
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 0.0, 100.0);
        let traj = m.optimal_trajectory(100.0, 1.0, 10);
        assert_eq!(traj.len(), 10);
        // 每步 10
        for &x in &traj {
            assert!((x - 10.0).abs() < 1e-9);
        }
        // 合计 = 100
        let total: f64 = traj.iter().sum();
        assert!((total - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_optimal_trajectory_risk_averse_front_loaded() {
        // 高 γ ⇒ 风险厌恶 ⇒ 前重后轻
        let m = AlmgrenChrissModel::new(0.5, 0.1, 0.1, 1.0, 100.0);
        let traj = m.optimal_trajectory(100.0, 1.0, 10);
        // 早期步数应 > 后期步数
        let early: f64 = traj.iter().take(3).sum();
        let late: f64 = traj.iter().rev().take(3).sum();
        assert!(early > late, "前 3 步之和 ({early}) 应 > 后 3 步之和 ({late})");
    }

    #[test]
    fn test_optimal_trajectory_sums_to_total() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 1e-6, 100.0);
        let traj = m.optimal_trajectory(500.0, 5.0, 20);
        let total: f64 = traj.iter().sum();
        // 浮点误差允许
        assert!((total - 500.0).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "总数量必须为正")]
    fn test_optimal_trajectory_rejects_zero_quantity() {
        let m = AlmgrenChrissModel::default();
        let _ = m.optimal_trajectory(0.0, 1.0, 10);
    }

    #[test]
    #[should_panic(expected = "总执行期必须为正")]
    fn test_optimal_trajectory_rejects_zero_time() {
        let m = AlmgrenChrissModel::default();
        let _ = m.optimal_trajectory(100.0, 0.0, 10);
    }

    #[test]
    #[should_panic(expected = "执行步数必须为正")]
    fn test_optimal_trajectory_rejects_zero_steps() {
        let m = AlmgrenChrissModel::default();
        let _ = m.optimal_trajectory(100.0, 1.0, 0);
    }

    #[test]
    fn test_expected_cost_uniform() {
        // 等分执行：ε × Σ x_k²/τ = ε × N × (Q/N)² / (T/N) = ε × Q²/T
        // ε × 100² / 1 = 100
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.0, 0.0, 100.0);
        let traj = vec![10.0; 10]; // Q=100, N=10, T=1
        let ec = m.expected_cost(&traj, 1.0);
        assert!((ec - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_expected_cost_with_permanent_impact() {
        // 临时 100 + 永久 0.001 × 10000 = 110
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 0.0, 100.0);
        let traj = vec![10.0; 10];
        let ec = m.expected_cost(&traj, 1.0);
        assert!((ec - 110.0).abs() < 1e-6);
    }

    #[test]
    fn test_cost_variance_uniform() {
        // σ² × τ × Σ (remaining)²
        // 每步 remaining = 100, 90, 80, ..., 10
        // Σ remaining² = 10000 + 8100 + ... + 100 = 3850
        // τ = 0.1
        // σ² × 0.1 × 3850 = 0.0004 × 385 = 1.54
        let m = AlmgrenChrissModel::new(0.02, 0.0, 0.0, 0.0, 100.0);
        let traj = vec![10.0; 10];
        let var = m.cost_variance(&traj, 1.0);
        // 使用步中点 remaining：100-5, 90-5, ..., 10-5 = 95, 85, ..., 5
        // Σ = 95² + 85² + ... + 5² = 9025 + 7225 + 5625 + 4225 + 3025 + 2025 + 1225 + 625 + 225 + 25 = 33250
        // var = 0.0004 × 0.1 × 33250 = 1.33
        assert!(var > 0.0);
        assert!(var < 2.0);
    }

    #[test]
    fn test_cost_variance_zero_quantity() {
        let m = AlmgrenChrissModel::default();
        assert_eq!(m.cost_variance(&[], 1.0), 0.0);
    }

    #[test]
    fn test_utility() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 1e-6, 100.0);
        let traj = vec![10.0; 10];
        let u = m.utility(&traj, 1.0);
        // u = E[C] + γ × Var[C] > E[C]
        let ec = m.expected_cost(&traj, 1.0);
        assert!(u > ec);
    }

    #[test]
    fn test_optimal_trajectory_more_front_loaded_with_higher_risk_aversion() {
        // 验证：风险厌恶 γ 越大，最优轨迹越 front-loaded
        // (用 first / last 比率衡量 front-load 程度)
        let m_low = AlmgrenChrissModel::new(0.1, 0.01, 0.001, 0.001, 100.0);
        let m_high = AlmgrenChrissModel::new(0.1, 0.01, 0.001, 1.0, 100.0);
        // low: κ = sqrt(0.001 × 0.01 / 0.00001) = 1.0
        // high: κ = sqrt(1.0 × 0.01 / 0.00001) = 31.6
        let traj_low = m_low.optimal_trajectory(100.0, 1.0, 10);
        let traj_high = m_high.optimal_trajectory(100.0, 1.0, 10);
        let ratio_low = traj_low[0] / traj_low.last().unwrap();
        let ratio_high = traj_high[0] / traj_high.last().unwrap();
        // 高风险厌恶 ⇒ first/last 比率更大 ⇒ 更 front-loaded
        assert!(
            ratio_high > ratio_low,
            "高 γ 的 first/last 比率 ({ratio_high}) 应 > 低 γ ({ratio_low})"
        );
    }

    #[test]
    fn test_implementation_shortfall_positive() {
        // 临时冲击使成交价 > arrival price
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.0, 0.0, 100.0);
        let traj = vec![10.0; 10];
        let is_v = m.implementation_shortfall(&traj, 1.0);
        assert!(is_v > 0.0);
    }

    #[test]
    fn test_implementation_shortfall_zero_for_zero_epsilon() {
        // ε = 0 ⇒ 无冲击 ⇒ IS = 0
        let m = AlmgrenChrissModel::new(0.02, 0.0, 0.0, 0.0, 100.0);
        let traj = vec![10.0; 10];
        let is_v = m.implementation_shortfall(&traj, 1.0);
        assert!(is_v.abs() < 1e-10);
    }

    #[test]
    fn test_implementation_shortfall_zero_trajectory() {
        let m = AlmgrenChrissModel::default();
        let is_v = m.implementation_shortfall(&[], 1.0);
        assert_eq!(is_v, 0.0);
    }

    #[test]
    fn test_execution_plan_from_trajectory() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 0.0, 100.0);
        let traj = vec![20.0, 20.0, 20.0, 20.0, 20.0]; // Q=100, 5 步
        let plan = ExecutionPlan::from_trajectory(&m, traj, 5.0);
        assert_eq!(plan.n_steps, 5);
        assert_eq!(plan.steps.len(), 5);
        assert!((plan.total_quantity - 100.0).abs() < 1e-9);
        // 每步 remaining 递减
        assert!((plan.steps[0].remaining - 80.0).abs() < 1e-9);
        assert!((plan.steps[1].remaining - 60.0).abs() < 1e-9);
        assert!((plan.steps[4].remaining - 0.0).abs() < 1e-9);
        // fill_price > arrival
        for step in &plan.steps {
            assert!(step.fill_price > 100.0);
        }
    }

    #[test]
    fn test_execution_plan_serde_roundtrip() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 1e-6, 100.0);
        let traj = m.optimal_trajectory(100.0, 1.0, 10);
        let plan = ExecutionPlan::from_trajectory(&m, traj, 1.0);
        let json = serde_json::to_string(&plan).unwrap();
        let de: ExecutionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan.n_steps, de.n_steps);
        assert_eq!(plan.steps.len(), de.steps.len());
    }

    #[test]
    fn test_model_serde_roundtrip() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 1e-6, 100.0);
        let json = serde_json::to_string(&m).unwrap();
        let de: AlmgrenChrissModel = serde_json::from_str(&json).unwrap();
        assert!((m.sigma - de.sigma).abs() < 1e-10);
        assert!((m.gamma - de.gamma).abs() < 1e-10);
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// N=1 ⇒ 全部一次性执行
    #[test]
    fn test_optimal_trajectory_single_step() {
        let m = AlmgrenChrissModel::default();
        let traj = m.optimal_trajectory(100.0, 1.0, 1);
        assert_eq!(traj.len(), 1);
        assert!((traj[0] - 100.0).abs() < 1e-9);
    }

    /// σ = 0 ⇒ κ = 0 ⇒ 等分
    #[test]
    fn test_zero_sigma_uniform_trajectory() {
        let m = AlmgrenChrissModel::new(0.0, 0.01, 0.001, 1e-6, 100.0);
        let traj = m.optimal_trajectory(100.0, 1.0, 10);
        for &x in &traj {
            assert!((x - 10.0).abs() < 1e-9);
        }
    }

    /// 极大 N（1000）应保持总和为 Q
    #[test]
    fn test_large_n_sums_correctly() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 1e-6, 100.0);
        let traj = m.optimal_trajectory(100.0, 1.0, 1000);
        let total: f64 = traj.iter().sum();
        assert!((total - 100.0).abs() < 1e-3);
    }

    /// 极小正数时间窗口
    #[test]
    fn test_epsilon_time() {
        let m = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 1e-6, 100.0);
        let traj = m.optimal_trajectory(100.0, f64::MIN_POSITIVE, 10);
        let total: f64 = traj.iter().sum();
        assert!((total - 100.0).abs() < 1e-6);
    }

    /// κ 极大 ⇒ 第一步执行几乎全部（趋近于立即执行）
    #[test]
    fn test_extreme_kappa_front_loads() {
        // 用大 σ / 小 ε / 大 γ 让 κ 很大
        let m = AlmgrenChrissModel::new(10.0, 0.001, 0.001, 100.0, 100.0);
        // κ = sqrt(100 × 100 / (0.001 × 0.001)) = sqrt(1e10) = 1e5
        let traj = m.optimal_trajectory(100.0, 1.0, 10);
        let first: f64 = traj[0];
        let last: f64 = *traj.last().unwrap();
        // 第一步远大于最后一步
        assert!(first > last * 10.0);
    }

    /// expected_cost 空轨迹 ⇒ 0
    #[test]
    fn test_expected_cost_empty() {
        let m = AlmgrenChrissModel::default();
        assert_eq!(m.expected_cost(&[], 1.0), 0.0);
    }

    /// expected_cost 临时 + 永久分项独立
    #[test]
    fn test_expected_cost_temp_plus_perm() {
        let m_full = AlmgrenChrissModel::new(0.02, 0.01, 0.001, 0.0, 100.0);
        let m_no_perm = AlmgrenChrissModel::new(0.02, 0.01, 0.0, 0.0, 100.0);
        let traj = vec![10.0; 10];
        let full = m_full.expected_cost(&traj, 1.0);
        let temp = m_no_perm.expected_cost(&traj, 1.0);
        // 永久部分 = full - temp = 0.001 × 100² = 10
        assert!((full - temp - 10.0).abs() < 1e-6);
    }
}
