//! 滚动窗口波动率估计器
//!
//! 维护固定大小窗口的收益率，用样本标准差估计波动率。
//!
//! # 公式
//!
//! `σ² = (1 / (n-1)) × Σ (r_i - r̄)²`
//!
//! # 复杂度
//!
//! - 空间：O(window) — 存储窗口内所有收益率
//! - 时间（update）：O(1) 摊销（用循环累加 + 替换最旧值）
//! - 时间（current_volatility）：O(window) — 需重新计算均值和方差
//!
//! 更高效的 O(1) 实现需要维护 Welford 算法的递推统计量。

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::error::{VolatilityError, VolatilityResult};
use super::estimator::VolatilityEstimator;

/// 滚动窗口波动率估计器
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollingVolatility {
    /// 窗口大小（必须 > 1，否则样本方差无定义）
    pub window: usize,
    /// 窗口内收益率
    buffer: VecDeque<f64>,
    /// 是否已就绪（至少有 2 个样本）
    ready: bool,
}

impl RollingVolatility {
    /// 创建滚动窗口估计器
    pub fn new(window: usize) -> VolatilityResult<Self> {
        if window < 2 {
            return Err(VolatilityError::ZeroWindow);
        }
        Ok(Self {
            window,
            buffer: VecDeque::with_capacity(window),
            ready: false,
        })
    }

    /// 返回窗口内样本数
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// 窗口是否已满
    pub fn is_full(&self) -> bool {
        self.buffer.len() == self.window
    }

    /// 返回当前样本方差
    pub fn variance(&self) -> VolatilityResult<f64> {
        if self.buffer.len() < 2 {
            return Err(VolatilityError::InsufficientData {
                required: 2,
                available: self.buffer.len(),
            });
        }
        let n = self.buffer.len() as f64;
        let mean: f64 = self.buffer.iter().sum::<f64>() / n;
        let var: f64 = self.buffer.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
        Ok(var)
    }
}

impl VolatilityEstimator for RollingVolatility {
    fn update(&mut self, return_value: f64) -> VolatilityResult<()> {
        if !return_value.is_finite() {
            return Err(VolatilityError::InvalidInput(format!(
                "收益率非有限：{return_value}"
            )));
        }
        if self.buffer.len() == self.window {
            self.buffer.pop_front();
        }
        self.buffer.push_back(return_value);
        self.ready = self.buffer.len() >= 2;
        Ok(())
    }

    fn current_volatility(&self) -> VolatilityResult<f64> {
        Ok(self.variance()?.sqrt())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn reset(&mut self) {
        self.buffer.clear();
        self.ready = false;
    }

    fn name(&self) -> &str {
        "RollingVolatility"
    }
}

impl Default for RollingVolatility {
    /// 默认滚动窗口：20 步
    fn default() -> Self {
        Self::new(20).expect("20 > 1 是有效窗口")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_valid_window() {
        let r = RollingVolatility::new(20).unwrap();
        assert_eq!(r.window, 20);
        assert_eq!(r.len(), 0);
        assert!(!r.is_full());
    }

    #[test]
    fn test_new_rejects_zero_window() {
        assert!(matches!(
            RollingVolatility::new(0),
            Err(VolatilityError::ZeroWindow)
        ));
    }

    #[test]
    fn test_new_rejects_one_window() {
        assert!(matches!(
            RollingVolatility::new(1),
            Err(VolatilityError::ZeroWindow)
        ));
    }

    #[test]
    fn test_default_uses_20_window() {
        let r = RollingVolatility::default();
        assert_eq!(r.window, 20);
    }

    #[test]
    fn test_update_fills_buffer() {
        let mut r = RollingVolatility::new(5).unwrap();
        for i in 0..5 {
            r.update(0.01 * (i as f64)).unwrap();
        }
        assert_eq!(r.len(), 5);
        assert!(r.is_full());
    }

    #[test]
    fn test_update_evicts_oldest_when_full() {
        let mut r = RollingVolatility::new(3).unwrap();
        r.update(0.10).unwrap();
        r.update(0.20).unwrap();
        r.update(0.30).unwrap();
        r.update(0.40).unwrap();
        // 0.10 被淘汰 ⇒ buffer = [0.20, 0.30, 0.40]
        assert_eq!(r.len(), 3);
        assert_eq!(r.buffer[0], 0.20);
        assert_eq!(r.buffer[2], 0.40);
    }

    #[test]
    fn test_update_rejects_nan() {
        let mut r = RollingVolatility::new(5).unwrap();
        assert!(matches!(
            r.update(f64::NAN),
            Err(VolatilityError::InvalidInput(_))
        ));
    }

    #[test]
    fn test_current_volatility_with_two_samples() {
        let mut r = RollingVolatility::new(5).unwrap();
        r.update(-0.01).unwrap();
        r.update(0.01).unwrap();
        // var = ((-0.01-0)² + (0.01-0)²) / 1 = 0.0002 ⇒ σ ≈ 0.01414
        let vol = r.current_volatility().unwrap();
        assert!(vol > 0.014 && vol < 0.015);
    }

    #[test]
    fn test_current_volatility_with_constant_returns() {
        let mut r = RollingVolatility::new(10).unwrap();
        for _ in 0..10 {
            r.update(0.05).unwrap();
        }
        // 所有样本相同 ⇒ 方差为 0
        assert!(r.current_volatility().unwrap().abs() < 1e-10);
    }

    #[test]
    fn test_current_volatility_before_update() {
        let r = RollingVolatility::new(5).unwrap();
        assert!(matches!(
            r.current_volatility(),
            Err(VolatilityError::InsufficientData { .. })
        ));
    }

    #[test]
    fn test_current_volatility_with_one_sample() {
        let mut r = RollingVolatility::new(5).unwrap();
        r.update(0.01).unwrap();
        assert!(matches!(
            r.current_volatility(),
            Err(VolatilityError::InsufficientData { .. })
        ));
    }

    #[test]
    fn test_variance_known_distribution() {
        // 构造已知分布：[1, 2, 3, 4, 5] ⇒ mean=3, var = ((1-3)² + (2-3)² + ... + (5-3)²)/4 = 10/4 = 2.5
        let mut r = RollingVolatility::new(5).unwrap();
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            r.update(v).unwrap();
        }
        let var = r.variance().unwrap();
        assert!((var - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_rolling_window_slides_correctly() {
        // 验证窗口滑动后方差正确更新
        let mut r = RollingVolatility::new(3).unwrap();
        // 初始窗口 [1, 2, 3]：mean=2, var = (1+0+1)/2 = 1
        r.update(1.0).unwrap();
        r.update(2.0).unwrap();
        r.update(3.0).unwrap();
        assert!((r.variance().unwrap() - 1.0).abs() < 1e-10);
        // 滑动后窗口 [2, 3, 4]：mean=3, var = 1
        r.update(4.0).unwrap();
        assert!((r.variance().unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_is_ready() {
        let mut r = RollingVolatility::new(5).unwrap();
        assert!(!r.is_ready());
        r.update(0.01).unwrap();
        assert!(!r.is_ready());
        r.update(0.02).unwrap();
        assert!(r.is_ready());
    }

    #[test]
    fn test_reset() {
        let mut r = RollingVolatility::new(3).unwrap();
        r.update(0.10).unwrap();
        r.update(0.20).unwrap();
        assert_eq!(r.len(), 2);
        r.reset();
        assert_eq!(r.len(), 0);
        assert!(!r.is_ready());
    }

    #[test]
    fn test_name() {
        let r = RollingVolatility::new(5).unwrap();
        assert_eq!(r.name(), "RollingVolatility");
    }

    #[test]
    fn test_serde_roundtrip() {
        let r = RollingVolatility::new(10).unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let de: RollingVolatility = serde_json::from_str(&json).unwrap();
        assert_eq!(r.window, de.window);
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 极大窗口（1000）应能处理
    #[test]
    fn test_large_window() {
        let mut r = RollingVolatility::new(1000).unwrap();
        for i in 0..2000 {
            r.update((i as f64 * 0.001).sin()).unwrap();
        }
        assert!(r.is_full());
        let vol = r.current_volatility().unwrap();
        assert!(vol > 0.0);
    }

    /// 交替正负收益率 ⇒ 较高波动率
    #[test]
    fn test_alternating_returns() {
        let mut r = RollingVolatility::new(4).unwrap();
        r.update(0.05).unwrap();
        r.update(-0.05).unwrap();
        r.update(0.05).unwrap();
        r.update(-0.05).unwrap();
        // mean=0, var = (0.0025 × 4) / 3 = 0.0033
        let var = r.variance().unwrap();
        assert!((var - 0.0025 * 4.0 / 3.0).abs() < 1e-10);
    }

    /// 零收益率窗口 ⇒ 零波动率
    #[test]
    fn test_zero_returns_window() {
        let mut r = RollingVolatility::new(10).unwrap();
        for _ in 0..10 {
            r.update(0.0).unwrap();
        }
        assert_eq!(r.variance().unwrap(), 0.0);
    }
}
