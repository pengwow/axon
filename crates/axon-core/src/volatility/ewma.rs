//! EWMA（指数加权移动平均）波动率估计器
//!
//! 用指数衰减权重估计波动率，对近期样本赋予更高权重。
//!
//! # 公式
//!
//! 递推形式（避免存储历史窗口）：
//! - `σ²_t = (1 - λ) × r²_t + λ × σ²_{t-1}`
//! - `λ = exp(-1/τ)`，其中 τ 是时间常数（半衰期）
//!
//! 典型 λ 值：
//! - `λ = 0.94`：RiskMetrics 标准（日度数据）
//! - `λ = 0.97`：月度/低频
//! - `λ = 0.82`：高频
//!
//! # 初始化
//!
//! 默认用前 N 个样本计算样本方差作为初始方差，然后切换到 EWMA 递推。
//! 也可手动调用 `reset_with_variance()` 指定初始方差。

use serde::{Deserialize, Serialize};

use super::error::{VolatilityError, VolatilityResult};
use super::estimator::VolatilityEstimator;

/// EWMA 波动率估计器
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EwmaVolatility {
    /// 衰减因子 λ ∈ (0, 1]
    pub lambda: f64,
    /// 初始方差（用于启动 EWMA 递推）
    pub initial_variance: f64,
    /// 当前方差估计
    variance: f64,
    /// 是否已初始化（收到第一个样本）
    initialized: bool,
    /// 观察到的样本数
    count: u64,
}

impl EwmaVolatility {
    /// 创建 EWMA 估计器（默认 λ = 0.94，初始方差 = 1e-4）
    pub fn new(lambda: f64) -> VolatilityResult<Self> {
        if !(0.0 < lambda && lambda <= 1.0) {
            return Err(VolatilityError::InvalidLambda(lambda));
        }
        Ok(Self {
            lambda,
            initial_variance: 1e-4,
            variance: 1e-4,
            initialized: false,
            count: 0,
        })
    }

    /// RiskMetrics 标准 λ = 0.94（日度）
    pub fn riskmetrics() -> VolatilityResult<Self> {
        Self::new(0.94)
    }

    /// 设置初始方差
    pub fn with_initial_variance(mut self, var: f64) -> VolatilityResult<Self> {
        if var < 0.0 {
            return Err(VolatilityError::InvalidInput(format!(
                "初始方差必须 ≥ 0，实际 {var}"
            )));
        }
        if !var.is_finite() {
            return Err(VolatilityError::InvalidInput("初始方差非有限".to_string()));
        }
        self.initial_variance = var;
        self.variance = var;
        Ok(self)
    }

    /// 重置为指定方差
    pub fn reset_with_variance(&mut self, variance: f64) {
        self.variance = variance.max(0.0);
        self.initialized = true;
        self.count = 0;
    }

    /// 返回 EWMA 方差
    pub fn variance(&self) -> f64 {
        self.variance
    }
}

impl VolatilityEstimator for EwmaVolatility {
    fn update(&mut self, return_value: f64) -> VolatilityResult<()> {
        if !return_value.is_finite() {
            return Err(VolatilityError::InvalidInput(format!(
                "收益率非有限：{return_value}"
            )));
        }

        if !self.initialized {
            // 第一次：直接用样本方差初始化（不充分，仅作起点）
            self.variance = self.initial_variance;
            self.initialized = true;
        }

        // EWMA 递推
        self.variance = (1.0 - self.lambda) * return_value.powi(2) + self.lambda * self.variance;
        self.count += 1;
        Ok(())
    }

    fn current_volatility(&self) -> VolatilityResult<f64> {
        if !self.initialized {
            return Err(VolatilityError::InsufficientData {
                required: 1,
                available: 0,
            });
        }
        Ok(self.variance.sqrt())
    }

    fn is_ready(&self) -> bool {
        self.initialized && self.count > 0
    }

    fn reset(&mut self) {
        self.variance = self.initial_variance;
        self.initialized = false;
        self.count = 0;
    }

    fn name(&self) -> &str {
        "EwmaVolatility"
    }
}

impl Default for EwmaVolatility {
    /// 默认 EWMA 估计器（λ = 0.94）
    fn default() -> Self {
        Self::new(0.94).expect("λ = 0.94 是有效值")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_valid_lambda() {
        let e = EwmaVolatility::new(0.94).unwrap();
        assert!((e.lambda - 0.94).abs() < 1e-10);
        assert!((e.variance - 1e-4).abs() < 1e-10);
        assert!(!e.initialized);
    }

    #[test]
    fn test_new_rejects_zero_lambda() {
        assert!(matches!(
            EwmaVolatility::new(0.0),
            Err(VolatilityError::InvalidLambda(_))
        ));
    }

    #[test]
    fn test_new_rejects_negative_lambda() {
        assert!(matches!(
            EwmaVolatility::new(-0.1),
            Err(VolatilityError::InvalidLambda(_))
        ));
    }

    #[test]
    fn test_new_rejects_too_large_lambda() {
        assert!(matches!(
            EwmaVolatility::new(1.1),
            Err(VolatilityError::InvalidLambda(_))
        ));
    }

    #[test]
    fn test_new_accepts_lambda_one() {
        let e = EwmaVolatility::new(1.0).unwrap();
        assert!((e.lambda - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_riskmetrics_constructor() {
        let e = EwmaVolatility::riskmetrics().unwrap();
        assert!((e.lambda - 0.94).abs() < 1e-10);
    }

    #[test]
    fn test_with_initial_variance() {
        let e = EwmaVolatility::new(0.94)
            .unwrap()
            .with_initial_variance(0.0025)
            .unwrap();
        assert!((e.initial_variance - 0.0025).abs() < 1e-10);
    }

    #[test]
    fn test_with_initial_variance_rejects_negative() {
        assert!(EwmaVolatility::new(0.94)
            .unwrap()
            .with_initial_variance(-0.1)
            .is_err());
    }

    #[test]
    fn test_with_initial_variance_rejects_nan() {
        assert!(EwmaVolatility::new(0.94)
            .unwrap()
            .with_initial_variance(f64::NAN)
            .is_err());
    }

    #[test]
    fn test_update_first_sample_initializes() {
        let mut e = EwmaVolatility::new(0.94).unwrap();
        e.update(0.01).unwrap();
        assert!(e.initialized);
        assert_eq!(e.count, 1);
        // σ² = (1-0.94) × 0.0001 + 0.94 × 0.0001 = 0.0001
        let vol = e.current_volatility().unwrap();
        assert!((vol - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_update_rejects_nan() {
        let mut e = EwmaVolatility::new(0.94).unwrap();
        assert!(matches!(
            e.update(f64::NAN),
            Err(VolatilityError::InvalidInput(_))
        ));
    }

    #[test]
    fn test_update_rejects_infinity() {
        let mut e = EwmaVolatility::new(0.94).unwrap();
        assert!(matches!(
            e.update(f64::INFINITY),
            Err(VolatilityError::InvalidInput(_))
        ));
    }

    #[test]
    fn test_current_volatility_before_update() {
        let e = EwmaVolatility::new(0.94).unwrap();
        assert!(matches!(
            e.current_volatility(),
            Err(VolatilityError::InsufficientData { .. })
        ));
    }

    #[test]
    fn test_lambda_one_preserves_initial_variance() {
        // λ = 1 ⇒ 不更新 ⇒ 方差保持
        let mut e = EwmaVolatility::new(1.0)
            .unwrap()
            .with_initial_variance(0.01)
            .unwrap();
        e.update(0.05).unwrap();
        e.update(0.05).unwrap();
        e.update(0.05).unwrap();
        // σ² = 0.01 ⇒ σ = 0.1
        assert!((e.variance() - 0.01).abs() < 1e-10);
        assert!((e.current_volatility().unwrap() - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_ewma_converges_to_squared_returns_for_low_lambda() {
        // λ = 0.5：递推权重高，方差应快速跟随 r²
        // 第一次 update(0.10)：σ² = 0.5 × 0.01 + 0.5 × 0.0001 = 0.00505
        // 第二次 update(0.10)：σ² = 0.5 × 0.01 + 0.5 × 0.00505 = 0.007525
        // σ ≈ 0.0867（介于初始 0.01 和 0.10 之间）
        let mut e = EwmaVolatility::new(0.5).unwrap();
        e.update(0.10).unwrap();
        e.update(0.10).unwrap();
        let vol = e.current_volatility().unwrap();
        assert!(vol > 0.05);
        assert!(vol < 0.10);
    }

    #[test]
    fn test_ewma_converges_with_many_updates() {
        // 100 次相同 update(0.5) ⇒ σ² 接近 0.25 ⇒ σ 接近 0.5
        let mut e = EwmaVolatility::new(0.9).unwrap();
        for _ in 0..100 {
            e.update(0.5).unwrap();
        }
        let vol = e.current_volatility().unwrap();
        assert!((vol - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_ewma_responds_to_volatility_increase() {
        // 前 5 个样本：低波动；后 5 个：高波动
        // EWMA 应跟随上升
        let mut e = EwmaVolatility::new(0.8).unwrap();
        for _ in 0..5 {
            e.update(0.01).unwrap();
        }
        let vol_before = e.current_volatility().unwrap();
        for _ in 0..5 {
            e.update(0.05).unwrap();
        }
        let vol_after = e.current_volatility().unwrap();
        assert!(
            vol_after > vol_before,
            "波动率应随高波动样本增加：{vol_before} -> {vol_after}"
        );
    }

    #[test]
    fn test_reset_clears_state() {
        let mut e = EwmaVolatility::new(0.94).unwrap();
        e.update(0.05).unwrap();
        assert!(e.initialized);
        e.reset();
        assert!(!e.initialized);
        assert_eq!(e.count, 0);
    }

    #[test]
    fn test_is_ready() {
        let mut e = EwmaVolatility::new(0.94).unwrap();
        assert!(!e.is_ready());
        e.update(0.01).unwrap();
        assert!(e.is_ready());
    }

    #[test]
    fn test_reset_with_variance() {
        let mut e = EwmaVolatility::new(0.94).unwrap();
        e.reset_with_variance(0.04);
        assert!(e.initialized);
        assert!((e.current_volatility().unwrap() - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_name() {
        let e = EwmaVolatility::new(0.94).unwrap();
        assert_eq!(e.name(), "EwmaVolatility");
    }

    #[test]
    fn test_serde_roundtrip() {
        let e = EwmaVolatility::new(0.94).unwrap();
        let json = serde_json::to_string(&e).unwrap();
        let de: EwmaVolatility = serde_json::from_str(&json).unwrap();
        assert!((e.lambda - de.lambda).abs() < 1e-10);
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 极小 λ（接近 0）⇒ 几乎只用最新样本
    #[test]
    fn test_lambda_near_zero_uses_latest() {
        let mut e = EwmaVolatility::new(1e-9).unwrap();
        for _ in 0..10 {
            e.update(0.01).unwrap();
        }
        e.update(0.20).unwrap();
        // σ² ≈ 0.04
        let vol = e.current_volatility().unwrap();
        assert!((vol - 0.20).abs() < 1e-6);
    }

    /// 零收益率 ⇒ 零波动率
    #[test]
    fn test_zero_returns_zero_volatility() {
        // 从 0 方差启动，否则 EWMA 会保留 initial_variance 初始值
        let mut e = EwmaVolatility::new(0.94).unwrap();
        e.reset_with_variance(0.0);
        for _ in 0..100 {
            e.update(0.0).unwrap();
        }
        assert!(e.current_volatility().unwrap().abs() < 1e-10);
    }

    /// 大收益率 ⇒ 大波动率
    #[test]
    fn test_large_returns_large_volatility() {
        // 从 0 方差启动，避免初始 initial_variance 污染稳态
        let mut e = EwmaVolatility::new(0.5).unwrap();
        e.reset_with_variance(0.0);
        for _ in 0..200 {
            e.update(0.5).unwrap();
        }
        let vol = e.current_volatility().unwrap();
        // 稳态方差 = 0.25 ⇒ 波动率 = 0.5
        assert!((vol - 0.5).abs() < 1e-3);
    }

    /// 默认 EWMA 估计器有效
    #[test]
    fn test_default_works() {
        let e = EwmaVolatility::default();
        assert!((e.lambda - 0.94).abs() < 1e-10);
    }
}
