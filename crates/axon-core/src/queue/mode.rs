//! 队列运行模式

use serde::{Deserialize, Serialize};

/// 队列运行模式
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QueueMode {
    /// 正常模式：按时间顺序出队
    #[default]
    Normal,
    /// 暂停模式：`next()` 返回 `None`
    Paused,
    /// 单步模式：`next()` 只出一个事件然后切回 `Paused`
    StepOnce,
}

impl QueueMode {
    /// 是否处于暂停状态（仅 `Paused` 视为暂停；`StepOnce` 允许出队一次）
    #[inline]
    pub fn is_paused(self) -> bool {
        matches!(self, Self::Paused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_normal() {
        assert_eq!(QueueMode::default(), QueueMode::Normal);
    }

    #[test]
    fn test_is_paused() {
        assert!(!QueueMode::Normal.is_paused());
        assert!(QueueMode::Paused.is_paused());
        // StepOnce 模式下 next 仍可出队一次，不算暂停
        assert!(!QueueMode::StepOnce.is_paused());
    }
}
