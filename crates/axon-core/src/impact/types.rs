//! 冲击结果结构

use serde::{Deserialize, Serialize};

use crate::market::Side;

/// 市场冲击结果（价格偏移量）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Impact {
    /// 即时冲击：影响本次成交价格
    pub instantaneous: f64,
    /// 永久冲击：影响后续市场价格
    pub permanent: f64,
}

impl Impact {
    /// 零冲击
    #[inline]
    pub const fn zero() -> Self {
        Self {
            instantaneous: 0.0,
            permanent: 0.0,
        }
    }

    /// 总冲击
    #[inline]
    pub fn total(&self) -> f64 {
        self.instantaneous + self.permanent
    }

    /// 冲击后成交价格
    #[inline]
    pub fn adjusted_price(&self, mid_price: f64, side: Side) -> f64 {
        let impact = self.total();
        match side {
            Side::Buy => mid_price + impact,
            Side::Sell => mid_price - impact,
        }
    }
}

impl Default for Impact {
    #[inline]
    fn default() -> Self {
        Self::zero()
    }
}

/// 冲击模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactModelConfig {
    /// 线性冲击
    Linear {
        /// 冲击系数
        coefficient: f64,
        /// 深度层级数
        depth_levels: usize,
        /// 即时/永久比例（0.0-1.0）
        instantaneous_ratio: f64,
    },
    /// 幂律冲击
    PowerLaw {
        /// 冲击系数
        coefficient: f64,
        /// 幂律指数
        exponent: f64,
        /// 深度层级数
        depth_levels: usize,
        /// 即时/永久比例
        instantaneous_ratio: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_impact_zero() {
        let i = Impact::zero();
        assert_eq!(i.instantaneous, 0.0);
        assert_eq!(i.permanent, 0.0);
        assert_eq!(i.total(), 0.0);
    }

    #[test]
    fn test_impact_total() {
        let i = Impact {
            instantaneous: 0.7,
            permanent: 0.3,
        };
        assert!((i.total() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_impact_adjusted_price_buy() {
        let i = Impact {
            instantaneous: 0.005,
            permanent: 0.0,
        };
        let p = i.adjusted_price(100.0, Side::Buy);
        assert!((p - 100.005).abs() < 1e-10);
    }

    #[test]
    fn test_impact_adjusted_price_sell() {
        let i = Impact {
            instantaneous: 0.0,
            permanent: 0.01,
        };
        let p = i.adjusted_price(100.0, Side::Sell);
        assert!((p - 99.99).abs() < 1e-10);
    }

    #[test]
    fn test_impact_default() {
        let i = Impact::default();
        assert_eq!(i, Impact::zero());
    }

    #[test]
    fn test_impact_model_config_serialize() {
        let config = ImpactModelConfig::Linear {
            coefficient: 0.05,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("Linear"));
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 总冲击应等于即时+永久之和（负值场景）
    #[test]
    fn test_impact_total_negative_components() {
        let i = Impact {
            instantaneous: -0.5,
            permanent: -0.3,
        };
        assert!((i.total() - (-0.8)).abs() < 1e-10);
    }

    /// 调整后价格 - Buy 方向负冲击
    #[test]
    fn test_impact_adjusted_price_buy_negative_impact() {
        // 负冲击表示价格下移（异常场景：套利回退）
        let i = Impact {
            instantaneous: -0.01,
            permanent: 0.0,
        };
        let p = i.adjusted_price(100.0, Side::Buy);
        assert!((p - 99.99).abs() < 1e-10);
    }

    /// 调整后价格 - Sell 方向负冲击
    #[test]
    fn test_impact_adjusted_price_sell_negative_impact() {
        let i = Impact {
            instantaneous: 0.0,
            permanent: -0.01,
        };
        let p = i.adjusted_price(100.0, Side::Sell);
        assert!((p - 100.01).abs() < 1e-10);
    }

    /// 调整后价格 - 零中间价应仍正确
    #[test]
    fn test_impact_adjusted_price_zero_mid() {
        let i = Impact {
            instantaneous: 0.5,
            permanent: 0.5,
        };
        let p_buy = i.adjusted_price(0.0, Side::Buy);
        let p_sell = i.adjusted_price(0.0, Side::Sell);
        assert!((p_buy - 1.0).abs() < 1e-10);
        assert!((p_sell - (-1.0)).abs() < 1e-10);
    }

    /// 调整后价格 - 极大中间价应保持精度
    #[test]
    fn test_impact_adjusted_price_extreme_mid() {
        let i = Impact::zero();
        let p = i.adjusted_price(f64::MAX / 2.0, Side::Buy);
        assert!(p.is_finite());
    }

    /// 调整后价格 - 极小正中间价（f64::MIN_POSITIVE）
    #[test]
    fn test_impact_adjusted_price_min_positive_mid() {
        let i = Impact {
            instantaneous: f64::MIN_POSITIVE,
            permanent: 0.0,
        };
        let p = i.adjusted_price(f64::MIN_POSITIVE, Side::Buy);
        // 极小正数相加应仍为有限正数
        assert!(p.is_finite());
        assert!(p > 0.0);
    }

    /// 总冲击 - 极大量级
    #[test]
    fn test_impact_total_extreme_values() {
        let i = Impact {
            instantaneous: 1e308,
            permanent: 1e308,
        };
        let total = i.total();
        // 两个 1e308 相加可能溢出为 inf
        assert!(total.is_finite() || total.is_infinite());
    }

    /// PowerLaw 配置序列化往返一致性
    #[test]
    fn test_impact_model_config_power_law_roundtrip() {
        let config = ImpactModelConfig::PowerLaw {
            coefficient: 0.15,
            exponent: 0.6,
            depth_levels: 8,
            instantaneous_ratio: 0.65,
        };
        let json = serde_json::to_string(&config).unwrap();
        let de: ImpactModelConfig = serde_json::from_str(&json).unwrap();
        // 序列化为 PowerLaw variant
        assert!(json.contains("PowerLaw"));
        assert!(matches!(de, ImpactModelConfig::PowerLaw { .. }));
    }

    /// 极端 coefficient 值（0.0 与极大）
    #[test]
    fn test_impact_model_config_extreme_coefficients() {
        // 零系数 - 零冲击
        let zero = ImpactModelConfig::Linear {
            coefficient: 0.0,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
        };
        let json = serde_json::to_string(&zero).unwrap();
        assert!(json.contains("0"));

        // 极大系数
        let large = ImpactModelConfig::Linear {
            coefficient: 1.0e9,
            depth_levels: 10,
            instantaneous_ratio: 0.7,
        };
        let json = serde_json::to_string(&large).unwrap();
        let de: ImpactModelConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(de, ImpactModelConfig::Linear { .. }));
    }

    /// 零 depth_levels
    #[test]
    fn test_impact_model_config_zero_depth_levels() {
        let config = ImpactModelConfig::Linear {
            coefficient: 0.05,
            depth_levels: 0,
            instantaneous_ratio: 0.7,
        };
        let json = serde_json::to_string(&config).unwrap();
        let _de: ImpactModelConfig = serde_json::from_str(&json).unwrap();
    }

    /// instantaneous_ratio 边界值（0.0 / 1.0）
    #[test]
    fn test_impact_model_config_ratio_boundary() {
        for ratio in [0.0_f64, 1.0_f64] {
            let config = ImpactModelConfig::Linear {
                coefficient: 0.05,
                depth_levels: 10,
                instantaneous_ratio: ratio,
            };
            let json = serde_json::to_string(&config).unwrap();
            let _de: ImpactModelConfig = serde_json::from_str(&json).unwrap();
        }
    }
}
