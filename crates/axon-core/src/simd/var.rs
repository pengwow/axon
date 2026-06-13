//! SIMD 加速的 VaR 分位数计算

/// 使用快速选择算法找第 k 小元素（避免全排序）
///
/// 时间复杂度：O(n) 平均 vs O(n log n) 全排序
/// 用于 VaR 计算中的分位数查找。
pub fn partial_sort_var(data: &mut [f64], k: usize) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let k = k.min(data.len() - 1);
    quickselect(data, k);
    data[k]
}

/// 快速选择算法：找到第 k 小的元素并将其放在正确位置
fn quickselect(data: &mut [f64], k: usize) {
    if data.len() <= 1 {
        return;
    }

    let pivot_idx = partition(data);
    if k == pivot_idx {
    } else if k < pivot_idx {
        quickselect(&mut data[..pivot_idx], k);
    } else {
        quickselect(&mut data[pivot_idx + 1..], k - pivot_idx - 1);
    }
}

/// Lomuto 分区方案
fn partition(data: &mut [f64]) -> usize {
    let len = data.len();
    let pivot = data[len - 1];
    let mut i = 0;

    for j in 0..len - 1 {
        if data[j] <= pivot {
            data.swap(i, j);
            i += 1;
        }
    }

    data.swap(i, len - 1);
    i
}

/// 历史模拟法 VaR（使用快速选择优化）
///
/// 95% 置信度下，只需要找到第 5 百分位数，
/// 不需要对整个数据集排序。
#[allow(dead_code)]
pub fn calculate_var_optimized(returns: &mut [f64], confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let k = ((1.0 - confidence) * returns.len() as f64) as usize;
    let k = k.min(returns.len() - 1);
    quickselect(returns, k);
    (-returns[k]).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_sort_var_basic() {
        let mut data = vec![0.05, -0.03, -0.01, 0.02, -0.05, 0.03, -0.02, 0.01];
        let k = 1; // 第 2 小的元素
        let result = partial_sort_var(&mut data, k);
        assert_eq!(result, -0.03);
    }

    #[test]
    fn test_partial_sort_var_empty() {
        let mut data: Vec<f64> = vec![];
        let result = partial_sort_var(&mut data, 0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_calculate_var_optimized() {
        let mut returns = vec![
            -0.05, -0.03, -0.01, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07,
        ];
        let var = calculate_var_optimized(&mut returns, 0.95);
        assert!(var > 0.0);
    }

    #[test]
    fn test_calculate_var_optimized_all_positive() {
        let mut returns = vec![0.01, 0.02, 0.03, 0.04, 0.05];
        let var = calculate_var_optimized(&mut returns, 0.95);
        assert_eq!(var, 0.0);
    }

    #[test]
    fn test_quickselect_correctness() {
        let mut data = vec![5.0, 3.0, 8.0, 1.0, 9.0, 2.0, 7.0, 4.0, 6.0];
        quickselect(&mut data, 4);
        assert_eq!(data[4], 5.0); // 第 5 小的元素
    }
}
