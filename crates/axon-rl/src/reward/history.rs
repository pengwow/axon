//! 历史回报追踪器（环形缓冲区）
//!
//! 基于 `VecDeque` 的固定容量 FIFO 缓冲区，按时间顺序保存最近 N 步的收益率。

use std::collections::VecDeque;

/// 固定容量环形缓冲区，存储最近 N 步的收益率
///
/// - `push` 是 O(1)（摊销）
/// - `as_slice` 总是返回按时间顺序的连续切片（必要时通过 `make_contiguous` 旋转）
#[derive(Debug, Clone)]
pub struct ReturnHistory {
    deque: VecDeque<f64>,
    capacity: usize,
    /// 用于返回连续切片时的旋转存储（仅 `as_slice` 借用时填满）
    contiguous: Vec<f64>,
}

impl ReturnHistory {
    /// 构造容量为 `window` 的环形缓冲区
    pub fn new(capacity: usize) -> Self {
        Self {
            deque: VecDeque::with_capacity(capacity.max(1)),
            capacity,
            contiguous: Vec::new(),
        }
    }

    /// 推入一个回报
    pub fn push(&mut self, ret: f64) {
        if self.capacity == 0 {
            return;
        }
        if self.deque.len() == self.capacity {
            self.deque.pop_front();
        }
        self.deque.push_back(ret);
    }

    /// 返回按时间顺序排列的连续切片
    ///
    /// 借用期会占用内部 `contiguous` 存储，因此连续两次 `as_slice` 调用需串行。
    pub fn as_slice(&mut self) -> &[f64] {
        self.contiguous.clear();
        self.contiguous.extend(self.deque.iter().copied());
        &self.contiguous
    }

    /// 复制为按时间顺序的 `Vec`（用于风险计算等需要 owned 数据的场景）
    pub fn to_vec(&self) -> Vec<f64> {
        self.deque.iter().copied().collect()
    }

    /// 当前元素数
    pub fn len(&self) -> usize {
        self.deque.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.deque.is_empty()
    }

    /// 窗口容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 重置为空
    pub fn clear(&mut self) {
        self.deque.clear();
        self.contiguous.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_history_is_empty() {
        let mut h = ReturnHistory::new(5);
        assert_eq!(h.len(), 0);
        assert!(h.is_empty());
        assert_eq!(h.capacity(), 5);
        assert!(h.as_slice().is_empty());
    }

    #[test]
    fn test_push_below_capacity() {
        let mut h = ReturnHistory::new(5);
        h.push(1.0);
        h.push(2.0);
        assert_eq!(h.len(), 2);
        assert_eq!(h.as_slice(), &[1.0, 2.0]);
    }

    #[test]
    fn test_push_at_capacity_wraps() {
        let mut h = ReturnHistory::new(3);
        h.push(1.0);
        h.push(2.0);
        h.push(3.0);
        h.push(4.0);
        assert_eq!(h.len(), 3);
        // 旧值 1 被覆盖，按时间顺序应是 [2, 3, 4]
        assert_eq!(h.as_slice(), &[2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_to_vec_preserves_order() {
        let mut h = ReturnHistory::new(4);
        h.push(0.1);
        h.push(0.2);
        h.push(0.3);
        h.push(0.4);
        h.push(0.5);
        assert_eq!(h.to_vec(), vec![0.2, 0.3, 0.4, 0.5]);
    }

    #[test]
    fn test_zero_capacity_no_panic() {
        let mut h = ReturnHistory::new(0);
        h.push(1.0);
        assert_eq!(h.len(), 0);
        assert!(h.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut h = ReturnHistory::new(3);
        h.push(1.0);
        h.push(2.0);
        h.clear();
        assert!(h.is_empty());
        // 清理后能再次写入
        h.push(9.0);
        assert_eq!(h.as_slice(), &[9.0]);
    }
}
