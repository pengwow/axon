//! 数量类型
//!
//! 与 `Price` 相同的 newtype 模式，但独立类型以防止数量与价格混淆。
//!
//! # 设计决策
//!
//! ## 为什么使用 `f64` 而非 `rust_decimal::Decimal`
//!
//! - **性能**：`f64` 是原生 CPU 类型，回测引擎需要处理百万级 Tick/Bar
//! - **生态兼容**：与主流 Rust 数值生态（`serde_json` / `bincode` / `arrow`）无缝
//! - **精度可接受**：回测业务通常容忍 1e-9 量级误差
//!
//! ## 允许负数
//!
//! 与 `Price` 不同，`Quantity` **允许负数**：投资组合的 `Position` 用负数
//! 表示空头持仓（`quantity < 0` ⇒ 空头；`quantity > 0` ⇒ 多头；`quantity = 0` ⇒ 空仓）。
//! 交易方向（买/卖）由 `Side` 决定，不通过数量符号表达。
//!
//! 负数**不影响** `Ord` 抑制的合理性 —— 负数与正数、负数与负数都是全可比较的，
//! 唯一需警惕的是 `NaN`（已被 `from_f64` 的 `is_finite()` 过滤）。
//!
//! ## `Eq` / `Ord` / `Hash` 手工实现的原因
//!
//! `f64` **根本性不实现 `Ord`**（仅 `PartialOrd`），因为 `f64` 存在 `NaN` 不可比较问题。
//! 撮合引擎需要 `BTreeMap<Quantity, ...>` 索引时，必须有全序关系。
//! 因此采用 **`#[derive(PartialEq, PartialOrd)]` + 手工 `impl Eq` + 手工 `impl Ord`** 的标准模式。
//!
//! ## NaN 安全性
//!
//! - `Quantity::from_f64` 使用 `is_finite()` 过滤 NaN 与 ±∞（但**不**限制符号，允许负数）
//! - `Ord::cmp` 的 `unwrap_or(Ordering::Equal)` 是 NaN 的最后一道防线
//! - `Hash` 使用 `f64::to_bits()` 而非直接 `self.0.hash(state)`，避免 NaN 哈希不稳定
//!
//! ## 何时应当重构
//!
//! 当以下条件之一满足时，应迁移到 `rust_decimal::Decimal`：
//! - 接入实盘交易，对精度有严格要求
//! - 跨语言/跨平台序列化出现精度漂移
//! - Rust 标准库为 `f64` 添加 `Ord` 实现（当前 RFC 2718 未通过）

use serde::{Deserialize, Serialize};

/// 数量类型（newtype 包装 `f64`）
/// `f64` 不实现 `Eq`/`Ord`/`Hash`，手工实现并保证与 `PartialEq`/`PartialOrd` 一致。
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Quantity(f64);

impl Quantity {
    /// 从 `f64` 构造
    ///
    /// 注：允许负数（`Position` 用负数表示空头持仓；交易方向由 `Side` 决定）
    #[inline]
    pub fn from_f64(v: f64) -> Self {
        Self(if v.is_finite() { v } else { 0.0 })
    }

    /// 转换为 `f64`
    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0
    }

    /// 是否为零
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0.0
    }
}

impl Default for Quantity {
    #[inline]
    fn default() -> Self {
        Self(0.0)
    }
}

impl Eq for Quantity {}

// ──────────────────────────────────────────────────────────────────────────────
// 警告抑制：`clippy::derive_ord_xor_partial_ord`
//
// 抑制原因：
//   `f64` 不实现 `Ord`（仅 `PartialOrd`）。撮合引擎需要 `BTreeMap<Price, ...>`
//   作为价格索引（见 axon-backtest::matching::engine.rs），相同模式适用于
//   `BTreeMap<Quantity, ...>` 作为持仓聚合索引。必须保留
//   `#[derive(PartialOrd)]` + 手工 `impl Ord` 的组合。
//
// 适用场景：
//   所有 `Quantity` 类型需要的全序关系（`<` / `>` / `BTreeMap` 键 / `BTreeSet` 元素）。
//
// 与 `Price` 的差异：
//   - `Quantity` 允许负数（空头持仓），但负数与正数、负数与负数、零都是全可比较的
//   - 不影响抑制的合理性 —— `unwrap_or(Equal)` 路径仅在 NaN 时触发，已被 `is_finite()` 阻断
//
// 潜在风险：
//   - 若 `Quantity::from_f64` 未妥善过滤 NaN，`partial_cmp` 返回 `None` 时会
//     退化为 `Equal`。**当前 `from_f64` 使用 `is_finite()` 过滤，已确保不含 NaN**。
//
// 未来优化：
//   当迁移到 `rust_decimal::Decimal` 或整数定标（i64 × 1e-8）时，可同时
//   移除 `#[allow]` 与手工 `impl Ord`，直接 derive。
//
// 临时措施追踪：CHANGELOG.md L142
// ──────────────────────────────────────────────────────────────────────────────
#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for Quantity {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // 构造时已保证非 NaN，可安全使用 partial_cmp
        // 保留 `unwrap_or` 作为防御性编程，应对未来可能的 NaN 渗入路径
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl std::hash::Hash for Quantity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // 使用 `to_bits()` 而非 `self.0.hash(state)`：
        //   - 避免 NaN 的 `f64::hash` 行为不稳定（不同 NaN 表示可能产生不同哈希）
        //   - 满足 `Hash + Eq` 一致性要求（Eq 已通过构造时过滤 NaN 实现）
        self.0.to_bits().hash(state);
    }
}

impl std::fmt::Display for Quantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<f64> for Quantity {
    #[inline]
    fn from(v: f64) -> Self {
        Self::from_f64(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantity_from_f64_roundtrip() {
        let q = Quantity::from_f64(10.0);
        assert!((q.as_f64() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_quantity_default_is_zero() {
        let q = Quantity::default();
        assert_eq!(q.as_f64(), 0.0);
    }

    #[test]
    fn test_quantity_addition() {
        let a = Quantity::from_f64(10.0);
        let b = Quantity::from_f64(5.0);
        let sum = Quantity::from_f64(a.as_f64() + b.as_f64());
        assert!((sum.as_f64() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_quantity_negative_allowed() {
        // Position 用负数表示空头持仓
        let q = Quantity::from_f64(-1.0);
        assert_eq!(q.as_f64(), -1.0);
    }

    #[test]
    fn test_quantity_comparison() {
        let a = Quantity::from_f64(10.0);
        let b = Quantity::from_f64(20.0);
        assert!(a < b);
    }

    #[test]
    fn test_quantity_from_impl() {
        let q: Quantity = 5.5_f64.into();
        assert!((q.as_f64() - 5.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_quantity_is_zero() {
        assert!(Quantity::default().is_zero());
        assert!(!Quantity::from_f64(0.001).is_zero());
    }

    // ─── 边界场景 ──────────────────────────────────────

    /// NaN 输入应归零
    #[test]
    fn test_quantity_nan_clamped_to_zero() {
        let q = Quantity::from_f64(f64::NAN);
        assert!(q.as_f64().is_finite());
        assert_eq!(q.as_f64(), 0.0);
        assert!(q.is_zero());
    }

    /// +∞ / -∞ 应归零
    #[test]
    fn test_quantity_infinity_clamped_to_zero() {
        assert_eq!(Quantity::from_f64(f64::INFINITY).as_f64(), 0.0);
        assert_eq!(Quantity::from_f64(f64::NEG_INFINITY).as_f64(), 0.0);
    }

    /// 极大正值应保留（Position 满仓多头）
    #[test]
    fn test_quantity_max_positive_preserved() {
        let q = Quantity::from_f64(f64::MAX);
        assert_eq!(q.as_f64(), f64::MAX);
    }

    /// 极大负值应保留（Position 满仓空头）
    #[test]
    fn test_quantity_max_negative_preserved() {
        let q = Quantity::from_f64(f64::MIN);
        assert_eq!(q.as_f64(), f64::MIN);
        assert!(q.as_f64() < 0.0);
    }

    /// 极小正数应保留
    #[test]
    fn test_quantity_min_positive_preserved() {
        let q = Quantity::from_f64(f64::MIN_POSITIVE);
        assert_eq!(q.as_f64(), f64::MIN_POSITIVE);
    }

    /// 零数量应保留
    #[test]
    fn test_quantity_zero_preserved() {
        let q = Quantity::from_f64(0.0);
        assert_eq!(q.as_f64(), 0.0);
        assert!(q.is_zero());
    }

    /// Hash 一致性（含负数）
    #[test]
    fn test_quantity_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        for v in [0.0_f64, 1.0, -1.0, f64::MAX, f64::MIN, f64::MIN_POSITIVE] {
            let a = Quantity::from_f64(v);
            let b = Quantity::from_f64(v);
            let mut ha = DefaultHasher::new();
            let mut hb = DefaultHasher::new();
            a.hash(&mut ha);
            b.hash(&mut hb);
            assert_eq!(ha.finish(), hb.finish(), "Quantity({v}) 哈希不一致");
        }
    }

    /// 负数与正数比较（Position 多空比较）
    #[test]
    fn test_quantity_negative_vs_positive_ordering() {
        let neg = Quantity::from_f64(-10.0);
        let zero = Quantity::from_f64(0.0);
        let pos = Quantity::from_f64(10.0);
        assert!(neg < zero);
        assert!(zero < pos);
        assert!(neg < pos);
    }
}
