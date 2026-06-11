//! 历史波动率估计器
//!
//! 为 [`AdaptiveImpactModel`](super::adaptive::AdaptiveImpactModel) 提供真实波动率输入。
//!
//! # 模块
//!
//! - [`estimator`]：[`VolatilityEstimator`] trait + 估计器枚举
//! - [`ewma`]：[`EwmaVolatility`] 指数加权移动平均估计器
//! - [`rolling`]：[`RollingVolatility`] 滚动窗口估计器
//! - [`garman_klass`]：[`GarmanKlassVolatility`] 基于 OHLC 的 Garman-Klass 估计器
//! - [`error`]：[`VolatilityError`] 错误类型

pub mod error;
pub mod estimator;
pub mod ewma;
pub mod garman_klass;
pub mod rolling;

pub use error::{VolatilityError, VolatilityResult};
pub use estimator::{VolatilityEstimator, VolatilitySource};
pub use ewma::EwmaVolatility;
pub use garman_klass::{GarmanKlassVolatility, OhlcBar};
pub use rolling::RollingVolatility;
