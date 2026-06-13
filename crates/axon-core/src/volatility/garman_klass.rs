//! Garman-Klass 波动率估计器
//!
//! 基于 OHLC（开高低收）数据的高效波动率估计器，利用区间内价格极值信息，
//! 比基于单一收盘价的估计更准确（5-8 倍效率）。
//!
//! # 公式
//!
//! Garman-Klass 1980 原始公式：
//! `σ² = 0.5 × (ln(H/L))² - (2 × ln(2) - 1) × (ln(C/O))²`
//!
//! Yang-Zhang 2000 扩展（结合 overnight returns）：
//! `σ² = σ_o² + k × σ_c² + (1-k) × σ_rs²`
//!
//! 本实现使用经典 Garman-Klass 公式（单周期）。
//!
//! # 适用场景
//!
//! - 高频日内数据（分钟/小时 K 线）
//! - 当 close-open drift 很小时估计最准
//! - 假设价格服从几何布朗运动

use serde::{Deserialize, Serialize};

use super::error::{VolatilityError, VolatilityResult};
use super::estimator::VolatilityEstimator;

/// 单根 K 线的 OHLC 数据
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OhlcBar {
    /// 开盘价
    pub open: f64,
    /// 最高价
    pub high: f64,
    /// 最低价
    pub low: f64,
    /// 收盘价
    pub close: f64,
}

impl OhlcBar {
    /// 验证 OHLC 数据合法性
    pub fn validate(&self) -> VolatilityResult<()> {
        if !self.open.is_finite() || self.open <= 0.0 {
            return Err(VolatilityError::InvalidInput(format!(
                "open 必须为正有限值，实际 {}",
                self.open
            )));
        }
        if !self.high.is_finite() || self.high <= 0.0 {
            return Err(VolatilityError::InvalidInput(format!(
                "high 必须为正有限值，实际 {}",
                self.high
            )));
        }
        if !self.low.is_finite() || self.low <= 0.0 {
            return Err(VolatilityError::InvalidInput(format!(
                "low 必须为正有限值，实际 {}",
                self.low
            )));
        }
        if !self.close.is_finite() || self.close <= 0.0 {
            return Err(VolatilityError::InvalidInput(format!(
                "close 必须为正有限值，实际 {}",
                self.close
            )));
        }
        if self.high < self.low {
            return Err(VolatilityError::InvalidInput(format!(
                "high ({}) < low ({})",
                self.high, self.low
            )));
        }
        if self.high < self.open || self.high < self.close {
            return Err(VolatilityError::InvalidInput(
                "high 必须 ≥ open 和 close".to_string(),
            ));
        }
        if self.low > self.open || self.low > self.close {
            return Err(VolatilityError::InvalidInput(
                "low 必须 ≤ open 和 close".to_string(),
            ));
        }
        Ok(())
    }
}

/// Garman-Klass 波动率估计器
///
/// 用单根 OHLC K 线更新内部方差估计。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GarmanKlassVolatility {
    /// 滑动平均窗口大小（必须 > 0）
    pub window: usize,
    /// 内部方差估计（按窗口滑动平均）
    variance_sum: f64,
    /// 已观察样本数
    count: usize,
    /// 上一次的 close（用于实现简单版本的"close-to-close"漂移项）
    prev_close: Option<f64>,
}

impl GarmanKlassVolatility {
    /// 创建 Garman-Klass 估计器
    ///
    /// # 参数
    ///
    /// - `window`：滑动平均窗口大小（> 0；1 = 无平滑）
    pub fn new(window: usize) -> VolatilityResult<Self> {
        if window == 0 {
            return Err(VolatilityError::ZeroWindow);
        }
        Ok(Self {
            window,
            variance_sum: 0.0,
            count: 0,
            prev_close: None,
        })
    }

    /// 返回已观察样本数
    pub fn count(&self) -> usize {
        self.count
    }

    /// 计算单根 K 线的 Garman-Klass 方差
    ///
    /// 公式：`σ² = 0.5 × (ln(H/L))² - (2 × ln(2) - 1) × (ln(C/O))²`
    pub fn gk_variance(bar: &OhlcBar) -> f64 {
        let log_hl = (bar.high / bar.low).ln();
        let log_co = (bar.close / bar.open).ln();
        let term1 = 0.5 * log_hl.powi(2);
        let term2 = (2.0 * 2.0_f64.ln() - 1.0) * log_co.powi(2);
        (term1 - term2).max(0.0)
    }
}

impl VolatilityEstimator for GarmanKlassVolatility {
    fn update(&mut self, return_value: f64) -> VolatilityResult<()> {
        // 直接 update 模式：把 return_value 当作 close-to-close 波动率
        // （OHLC 模式用 update_bar）
        if !return_value.is_finite() {
            return Err(VolatilityError::InvalidInput(format!(
                "收益率非有限：{return_value}"
            )));
        }
        self.count += 1;
        // 简单累加：方差估计 = r²
        if self.count > self.window {
            // 简单：超过窗口后只保留最新
            self.variance_sum = return_value.powi(2) * self.window as f64;
        } else {
            self.variance_sum += return_value.powi(2);
        }
        Ok(())
    }

    fn current_volatility(&self) -> VolatilityResult<f64> {
        if self.count == 0 {
            return Err(VolatilityError::InsufficientData {
                required: 1,
                available: 0,
            });
        }
        let n = self.count.min(self.window) as f64;
        Ok((self.variance_sum / n).sqrt())
    }

    fn is_ready(&self) -> bool {
        self.count > 0
    }

    fn reset(&mut self) {
        self.variance_sum = 0.0;
        self.count = 0;
        self.prev_close = None;
    }

    fn name(&self) -> &str {
        "GarmanKlassVolatility"
    }
}

impl GarmanKlassVolatility {
    /// 用 OHLC K 线更新
    pub fn update_bar(&mut self, bar: OhlcBar) -> VolatilityResult<()> {
        bar.validate()?;
        let var = Self::gk_variance(&bar);
        // 滑动平均累加
        if self.count >= self.window {
            // 简化的滑动平均：减去最早值加上新值（实际应维护循环缓冲）
            // 此处采用 EMA 近似：(1 - 1/window) × old + new / window
            let alpha = 1.0 / self.window as f64;
            self.variance_sum =
                (1.0 - alpha) * self.variance_sum + alpha * var * self.window as f64;
        } else {
            self.variance_sum += var;
            self.count += 1;
        }
        self.prev_close = Some(bar.close);
        Ok(())
    }
}

impl Default for GarmanKlassVolatility {
    /// 默认窗口：20 根 K 线
    fn default() -> Self {
        Self::new(20).expect("20 > 0 是有效窗口")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bar() -> OhlcBar {
        OhlcBar {
            open: 100.0,
            high: 102.0,
            low: 99.0,
            close: 101.0,
        }
    }

    #[test]
    fn test_ohlc_bar_validate_ok() {
        assert!(sample_bar().validate().is_ok());
    }

    #[test]
    fn test_ohlc_bar_validate_negative_open() {
        let bar = OhlcBar {
            open: -1.0,
            ..sample_bar()
        };
        assert!(bar.validate().is_err());
    }

    #[test]
    fn test_ohlc_bar_validate_zero_high() {
        let bar = OhlcBar {
            high: 0.0,
            ..sample_bar()
        };
        assert!(bar.validate().is_err());
    }

    #[test]
    fn test_ohlc_bar_validate_high_less_than_low() {
        let bar = OhlcBar {
            high: 99.0,
            low: 100.0,
            ..sample_bar()
        };
        assert!(bar.validate().is_err());
    }

    #[test]
    fn test_ohlc_bar_validate_high_less_than_open() {
        let bar = OhlcBar {
            open: 105.0,
            high: 100.0,
            ..sample_bar()
        };
        assert!(bar.validate().is_err());
    }

    #[test]
    fn test_ohlc_bar_validate_low_greater_than_close() {
        let bar = OhlcBar {
            close: 95.0,
            low: 100.0,
            ..sample_bar()
        };
        assert!(bar.validate().is_err());
    }

    #[test]
    fn test_gk_variance_known() {
        // H/L = 1.02, C/O = 1.01：直接用 high=1.02, low=1.0 让 high/low=1.02
        let bar = OhlcBar {
            open: 1.0,
            high: 1.02,
            low: 1.0,
            close: 1.01,
        };
        let var = GarmanKlassVolatility::gk_variance(&bar);
        // 0.5 × (ln 1.02)² - (2ln2-1) × (ln 1.01)²
        let log_hl = 1.02_f64.ln();
        let log_co = 1.01_f64.ln();
        let expected = 0.5 * log_hl.powi(2) - (2.0 * 2.0_f64.ln() - 1.0) * log_co.powi(2);
        assert!((var - expected).abs() < 1e-10);
    }

    #[test]
    fn test_gk_variance_zero_for_unchanged() {
        // O=H=L=C ⇒ 0
        let bar = OhlcBar {
            open: 100.0,
            high: 100.0,
            low: 100.0,
            close: 100.0,
        };
        assert_eq!(GarmanKlassVolatility::gk_variance(&bar), 0.0);
    }

    #[test]
    fn test_gk_variance_non_negative_for_unfavorable_drift() {
        // 当 C << O 时，第二项可能超过第一项
        // 此时 GK 方差被 clamp 到 0
        let bar = OhlcBar {
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 90.0,
        };
        let var = GarmanKlassVolatility::gk_variance(&bar);
        assert!(var >= 0.0);
    }

    #[test]
    fn test_new_with_valid_window() {
        let g = GarmanKlassVolatility::new(20).unwrap();
        assert_eq!(g.window, 20);
        assert_eq!(g.count(), 0);
    }

    #[test]
    fn test_new_rejects_zero_window() {
        assert!(matches!(
            GarmanKlassVolatility::new(0),
            Err(VolatilityError::ZeroWindow)
        ));
    }

    #[test]
    fn test_default_uses_20_window() {
        let g = GarmanKlassVolatility::default();
        assert_eq!(g.window, 20);
    }

    #[test]
    fn test_update_bar_accumulates() {
        let mut g = GarmanKlassVolatility::new(10).unwrap();
        g.update_bar(sample_bar()).unwrap();
        g.update_bar(sample_bar()).unwrap();
        assert_eq!(g.count(), 2);
    }

    #[test]
    fn test_update_bar_rejects_invalid() {
        let mut g = GarmanKlassVolatility::new(10).unwrap();
        let bad = OhlcBar {
            open: -1.0,
            ..sample_bar()
        };
        assert!(g.update_bar(bad).is_err());
    }

    #[test]
    fn test_current_volatility_after_one_bar() {
        let mut g = GarmanKlassVolatility::new(10).unwrap();
        g.update_bar(sample_bar()).unwrap();
        let vol = g.current_volatility().unwrap();
        assert!(vol > 0.0);
    }

    #[test]
    fn test_current_volatility_before_update() {
        let g = GarmanKlassVolatility::new(10).unwrap();
        assert!(matches!(
            g.current_volatility(),
            Err(VolatilityError::InsufficientData { .. })
        ));
    }

    #[test]
    fn test_reset() {
        let mut g = GarmanKlassVolatility::new(10).unwrap();
        g.update_bar(sample_bar()).unwrap();
        g.reset();
        assert_eq!(g.count(), 0);
        assert!(!g.is_ready());
    }

    #[test]
    fn test_name() {
        let g = GarmanKlassVolatility::new(10).unwrap();
        assert_eq!(g.name(), "GarmanKlassVolatility");
    }

    #[test]
    fn test_serde_roundtrip() {
        let g = GarmanKlassVolatility::new(20).unwrap();
        let json = serde_json::to_string(&g).unwrap();
        let de: GarmanKlassVolatility = serde_json::from_str(&json).unwrap();
        assert_eq!(g.window, de.window);
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 真实场景：连续 5 根温和上涨 K 线
    #[test]
    fn test_realistic_ohlc_sequence() {
        let mut g = GarmanKlassVolatility::new(5).unwrap();
        for i in 0..5 {
            let p = 100.0 + i as f64;
            let bar = OhlcBar {
                open: p,
                high: p + 1.5,
                low: p - 1.0,
                close: p + 0.5,
            };
            g.update_bar(bar).unwrap();
        }
        let vol = g.current_volatility().unwrap();
        assert!(vol > 0.0);
        assert!(vol < 0.1); // 温和波动
    }

    /// 价格序列极端（高波幅）
    #[test]
    fn test_high_volatility_bar() {
        let mut g = GarmanKlassVolatility::new(1).unwrap();
        let bar = OhlcBar {
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
        };
        g.update_bar(bar).unwrap();
        let vol = g.current_volatility().unwrap();
        // (ln 110/90)² × 0.5 = (0.2)² × 0.5 = 0.02 ⇒ vol ≈ 0.14
        assert!(vol > 0.1);
    }

    /// NaN 价格 ⇒ 错误
    #[test]
    fn test_nan_price_in_bar() {
        let mut g = GarmanKlassVolatility::new(10).unwrap();
        let bar = OhlcBar {
            open: f64::NAN,
            ..sample_bar()
        };
        assert!(g.update_bar(bar).is_err());
    }
}
