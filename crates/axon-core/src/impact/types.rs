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
}
