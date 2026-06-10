//! 时间精度枚举
//!
//! 用于 [`Timestamp::truncate`](super::Timestamp::truncate) 截断到指定精度。

use serde::{Deserialize, Serialize};

/// 时间精度等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimePrecision {
    /// 秒级精度（10^9 ns）
    Seconds,
    /// 毫秒级精度（10^6 ns）
    Millis,
    /// 微秒级精度（10^3 ns）
    Micros,
    /// 纳秒级精度（1 ns）
    Nanos,
}

impl TimePrecision {
    /// 返回该精度对应的纳秒除数
    #[inline]
    pub(crate) fn divisor_nanos(self) -> i64 {
        match self {
            Self::Seconds => 1_000_000_000,
            Self::Millis => 1_000_000,
            Self::Micros => 1_000,
            Self::Nanos => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_divisor_values() {
        assert_eq!(TimePrecision::Seconds.divisor_nanos(), 1_000_000_000);
        assert_eq!(TimePrecision::Millis.divisor_nanos(), 1_000_000);
        assert_eq!(TimePrecision::Micros.divisor_nanos(), 1_000);
        assert_eq!(TimePrecision::Nanos.divisor_nanos(), 1);
    }
}
