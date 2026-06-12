//! ActionProbabilities 单元测试
//!
//! 验证概率分布归一化、向量转换和基本构造。

use axon_ensemble::types::ActionProbabilities;

#[test]
fn test_action_probabilities_normalizes_to_sum_one() {
    // 原始概率 (2, 4, 4) — 总和 10
    let probs = ActionProbabilities::new(2.0, 4.0, 4.0);
    let sum = probs.buy + probs.sell + probs.hold;
    assert!(
        (sum - 1.0).abs() < 1e-9,
        "归一化后总和应为 1.0，实际 {}",
        sum
    );
}

#[test]
fn test_action_probabilities_preserves_relative_ratios() {
    // 原始 (1, 2, 1) → buy:hold = 1:1, sell = 2x
    let probs = ActionProbabilities::new(1.0, 2.0, 1.0);
    // 归一化后 buy=0.25, sell=0.5, hold=0.25
    assert!((probs.buy - 0.25).abs() < 1e-9);
    assert!((probs.sell - 0.5).abs() < 1e-9);
    assert!((probs.hold - 0.25).abs() < 1e-9);
}

#[test]
fn test_action_probabilities_to_vec_returns_three_elements() {
    let probs = ActionProbabilities::new(0.3, 0.4, 0.3);
    let v = probs.to_vec();
    assert_eq!(v.len(), 3);
    // 顺序：buy, sell, hold
    assert!((v[0] - 0.3).abs() < 1e-9);
    assert!((v[1] - 0.4).abs() < 1e-9);
    assert!((v[2] - 0.3).abs() < 1e-9);
}

#[test]
fn test_action_probabilities_handles_zero_total() {
    // 总和为 0 — 退化为均匀分布
    let probs = ActionProbabilities::new(0.0, 0.0, 0.0);
    let sum = probs.buy + probs.sell + probs.hold;
    assert!((sum - 1.0).abs() < 1e-9, "全 0 应退化为均匀分布，总和={}", sum);
}
