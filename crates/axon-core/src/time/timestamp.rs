//! 纳秒精度时间戳
//!
//! 存储 Unix 纪元（1970-01-01T00:00:00Z）以来的纳秒数，
//! 使用 `i64` 可表示约 ±292 年范围。
//!
//! TDD 规范：[`axon-design/01-tdd/01-phase1-core/01-timestamp.md`](../../../../../axon-design/01-tdd/01-phase1-core/01-timestamp.md)
//!
//! # 示例
//!
//! ```
//! use axon_core::time::{Timestamp, TimePrecision};
//! use std::time::Duration;
//!
//! // 基础构造与算术
//! let t1 = Timestamp::now();
//! let t2 = t1 + Duration::from_secs(60);
//! assert!(t2.is_after(&t1));
//!
//! // 序列化（serde transparent → 直接输出 i64）
//! let ts = Timestamp::from_millis(1_700_000_000_123);
//! let json = serde_json::to_string(&ts).unwrap();
//! assert_eq!(json, "1700000000123000000");
//!
//! // 精度截断
//! let truncated = ts.truncate(TimePrecision::Seconds);
//! assert_eq!(truncated.nanos, 1_700_000_000_000_000_000);
//! ```

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::precision::TimePrecision;

/// 纳秒精度时间戳
///
/// 使用 `#[serde(transparent)]` 直接序列化为 `i64`，
/// 例如 JSON 形式 `"1700000000123000000"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp {
    /// Unix 纪元纳秒数
    pub nanos: i64,
}

/// 时间戳模块错误
#[derive(Debug, Clone, Error)]
pub enum TimestampError {
    /// 纳秒值超出 `i64` 表示范围
    #[error("纳秒溢出：值 {value} 超出 i64 范围")]
    NanosOverflow {
        /// 溢出的原始值
        value: i128,
    },

    /// 无效时间戳
    #[error("无效时间戳：{0}")]
    Invalid(String),
}

/// 时间戳模块的 `Result` 别名
pub type TimestampResult<T> = std::result::Result<T, TimestampError>;

// ─── 构造方法 ─────────────────────────────────────────

impl Timestamp {
    /// 获取当前 UTC 时间戳
    #[inline]
    pub fn now() -> Self {
        let dt = chrono::Utc::now();
        Self {
            nanos: dt.timestamp_nanos_opt().unwrap_or(0),
        }
    }

    /// 从纳秒构造
    #[inline]
    pub const fn from_nanos(nanos: i64) -> Self {
        Self { nanos }
    }

    /// 从微秒构造（溢出时 panic）
    #[inline]
    pub const fn from_micros(micros: i64) -> Self {
        Self {
            nanos: match micros.checked_mul(1_000) {
                Some(n) => n,
                None => panic!("微秒转纳秒溢出"),
            },
        }
    }

    /// 从毫秒构造（溢出时 panic）
    #[inline]
    pub const fn from_millis(millis: i64) -> Self {
        Self {
            nanos: match millis.checked_mul(1_000_000) {
                Some(n) => n,
                None => panic!("毫秒转纳秒溢出"),
            },
        }
    }

    /// 从秒构造（溢出时 panic）
    #[inline]
    pub const fn from_secs(secs: i64) -> Self {
        Self {
            nanos: match secs.checked_mul(1_000_000_000) {
                Some(n) => n,
                None => panic!("秒转纳秒溢出"),
            },
        }
    }

    /// 从 chrono `DateTime<Utc>` 构造
    pub fn from_datetime(dt: &chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            nanos: dt.timestamp_nanos_opt().unwrap_or(0),
        }
    }

    /// 转换为 chrono `DateTime<Utc>`
    pub fn to_datetime(&self) -> chrono::DateTime<chrono::Utc> {
        let secs = self.nanos.div_euclid(1_000_000_000);
        let nanos_remainder = self.nanos.rem_euclid(1_000_000_000) as u32;
        chrono::DateTime::from_timestamp(secs, nanos_remainder)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("epoch 一定合法"))
    }

    /// 计算两个时间戳之间的绝对 `Duration`
    ///
    /// 使用 `abs_diff` 保证结果为非负值
    #[inline]
    pub fn duration_since(&self, other: &Timestamp) -> Duration {
        let diff_nanos = self.nanos.abs_diff(other.nanos);
        Duration::from_nanos(diff_nanos)
    }

    /// 加上 `Duration`（溢出时 panic）
    #[inline]
    pub fn add(&self, duration: Duration) -> Timestamp {
        let add_nanos = duration.as_nanos() as i64;
        Timestamp {
            nanos: match self.nanos.checked_add(add_nanos) {
                Some(n) => n,
                None => panic!("Timestamp 加法溢出"),
            },
        }
    }

    /// 减去 `Duration`（下溢时 panic）
    #[inline]
    pub fn sub(&self, duration: Duration) -> Timestamp {
        let sub_nanos = duration.as_nanos() as i64;
        Timestamp {
            nanos: match self.nanos.checked_sub(sub_nanos) {
                Some(n) => n,
                None => panic!("Timestamp 减法溢出"),
            },
        }
    }

    /// 是否在 `other` 之前
    #[inline]
    pub fn is_before(&self, other: &Timestamp) -> bool {
        self.nanos < other.nanos
    }

    /// 是否在 `other` 之后
    #[inline]
    pub fn is_after(&self, other: &Timestamp) -> bool {
        self.nanos > other.nanos
    }

    /// 截断到指定精度
    pub fn truncate(&self, precision: TimePrecision) -> Timestamp {
        let divisor = precision.divisor_nanos();
        Timestamp {
            nanos: (self.nanos / divisor) * divisor,
        }
    }

    /// 序列化为 RFC 3339 字符串（人类可读）
    pub fn to_rfc3339(&self) -> String {
        self.to_datetime().to_rfc3339()
    }

    /// 从 RFC 3339 字符串反序列化
    pub fn from_rfc3339(s: &str) -> TimestampResult<Self> {
        let dt = chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|e| TimestampError::Invalid(format!("RFC 3339 解析失败: {e}")))?;
        Ok(Self::from_datetime(&dt.with_timezone(&chrono::Utc)))
    }
}

// ─── 运算符重载 ───────────────────────────────────────

impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn add(self, rhs: Duration) -> Timestamp {
        // 使用完全限定语法调用 inherent 方法，避免与 trait 方法名冲突导致的递归
        Timestamp::add(&self, rhs)
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn sub(self, rhs: Duration) -> Timestamp {
        Timestamp::sub(&self, rhs)
    }
}

impl Sub<Timestamp> for Timestamp {
    type Output = Duration;

    #[inline]
    fn sub(self, rhs: Timestamp) -> Duration {
        self.duration_since(&rhs)
    }
}

impl AddAssign<Duration> for Timestamp {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        let add_nanos = rhs.as_nanos() as i64;
        self.nanos = match self.nanos.checked_add(add_nanos) {
            Some(n) => n,
            None => panic!("Timestamp 加法溢出"),
        };
    }
}

impl SubAssign<Duration> for Timestamp {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        let sub_nanos = rhs.as_nanos() as i64;
        self.nanos = match self.nanos.checked_sub(sub_nanos) {
            Some(n) => n,
            None => panic!("Timestamp 减法溢出"),
        };
    }
}

impl Ord for Timestamp {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.nanos.cmp(&other.nanos)
    }
}

impl PartialOrd for Timestamp {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ─── Display ───────────────────────────────────────────

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dt = self.to_datetime();
        write!(f, "{}", dt.format("%Y-%m-%dT%H:%M:%S%.9fZ"))
    }
}

// ─── Default ───────────────────────────────────────────

impl Default for Timestamp {
    /// 默认值为 Unix 纪元（1970-01-01T00:00:00Z）
    #[inline]
    fn default() -> Self {
        Self { nanos: 0 }
    }
}

// ─── 测试 ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ─── 基础构造 ───────────────────────────────────────

    #[test]
    fn test_timestamp_from_nanos_roundtrip() {
        let ts = Timestamp::from_nanos(1_700_000_000_123_456_789);
        assert_eq!(ts.nanos, 1_700_000_000_123_456_789);
    }

    #[test]
    fn test_timestamp_now_is_reasonable() {
        // 当前时间应介于 2020-01-01 与 2030-01-01 之间（纳秒）
        let ts = Timestamp::now();
        let lower = Timestamp::from_nanos(1_577_836_800_000_000_000); // 2020-01-01
        let upper = Timestamp::from_nanos(1_893_456_000_000_000_000); // 2030-01-01
        assert!(ts.nanos > lower.nanos, "now 早于 2020: {}", ts.nanos);
        assert!(ts.nanos < upper.nanos, "now 晚于 2030: {}", ts.nanos);
    }

    #[test]
    fn test_timestamp_comparison() {
        let a = Timestamp::from_nanos(100);
        let b = Timestamp::from_nanos(200);
        assert!(a.is_before(&b));
        assert!(b.is_after(&a));
        assert!(!a.is_after(&b));
        assert_eq!(a.cmp(&b), Ordering::Less);
    }

    // ─── 时间算术 ───────────────────────────────────────

    #[test]
    fn test_timestamp_add_duration() {
        let ts = Timestamp::from_nanos(1_000);
        let result = ts.add(Duration::from_millis(500));
        assert_eq!(result.nanos, 1_000 + 500_000_000);
    }

    #[test]
    fn test_timestamp_sub_duration() {
        let ts = Timestamp::from_nanos(1_000_000_000_000);
        let result = ts.sub(Duration::from_micros(1_500));
        assert_eq!(result.nanos, 1_000_000_000_000 - 1_500_000);
    }

    #[test]
    fn test_duration_between_two_timestamps() {
        let a = Timestamp::from_nanos(1);
        let b = Timestamp::from_nanos(1_000_000_000_001);
        let d = b.duration_since(&a);
        assert_eq!(d, Duration::from_nanos(1_000_000_000_000));
    }

    // ─── 序列化 ────────────────────────────────────────

    #[test]
    fn test_timestamp_json_serialization() {
        let ts = Timestamp::from_millis(1_700_000_000_123);
        let json = serde_json::to_string(&ts).unwrap();
        assert_eq!(json, "1700000000123000000");
        let restored: Timestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(ts, restored);
    }

    #[test]
    fn test_timestamp_bincode_serialization() {
        let ts = Timestamp::from_nanos(0x0102_0304_0506_0708);
        let bytes = bincode::serialize(&ts).unwrap();
        assert_eq!(bytes.len(), 8);
        let restored: Timestamp = bincode::deserialize(&bytes).unwrap();
        assert_eq!(ts, restored);
    }

    #[test]
    fn test_timestamp_deserialization_invalid_input() {
        // 非数字 JSON 应解析失败
        let result: Result<Timestamp, _> = serde_json::from_str("\"not a number\"");
        assert!(result.is_err());
    }

    // ─── 边界 ──────────────────────────────────────────

    #[test]
    fn test_timestamp_min_value() {
        let ts = Timestamp::from_nanos(i64::MIN);
        assert_eq!(ts.nanos, i64::MIN);
    }

    #[test]
    fn test_timestamp_max_value() {
        let ts = Timestamp::from_nanos(i64::MAX);
        assert_eq!(ts.nanos, i64::MAX);
    }

    #[test]
    #[should_panic(expected = "加法溢出")]
    fn test_timestamp_overflow_panics() {
        let ts = Timestamp::from_nanos(i64::MAX);
        let _ = ts.add(Duration::from_nanos(1));
    }

    // ─── 精度截断 ──────────────────────────────────────

    #[test]
    fn test_timestamp_truncate_to_seconds() {
        let ts = Timestamp::from_nanos(1_700_000_000_123_456_789);
        let truncated = ts.truncate(TimePrecision::Seconds);
        assert_eq!(truncated.nanos, 1_700_000_000_000_000_000);
    }

    #[test]
    fn test_timestamp_truncate_to_millis() {
        let ts = Timestamp::from_nanos(1_700_000_000_123_456_789);
        let truncated = ts.truncate(TimePrecision::Millis);
        assert_eq!(truncated.nanos, 1_700_000_000_123_000_000);
    }

    // ─── 运算符重载 ─────────────────────────────────────

    #[test]
    fn test_timestamp_operator_add() {
        let ts = Timestamp::from_nanos(100);
        let result = ts + Duration::from_micros(1);
        assert_eq!(result.nanos, 100 + 1_000);
    }

    #[test]
    fn test_timestamp_operator_sub_yields_duration() {
        let a = Timestamp::from_nanos(2_000);
        let b = Timestamp::from_nanos(500);
        let d: Duration = a - b;
        assert_eq!(d, Duration::from_nanos(1_500));
    }

    // ─── RFC 3339 互转 ────────────────────────────────

    #[test]
    fn test_timestamp_rfc3339_roundtrip() {
        let ts = Timestamp::from_nanos(1_700_000_000_123_456_789);
        let s = ts.to_rfc3339();
        let restored = Timestamp::from_rfc3339(&s).unwrap();
        assert_eq!(ts, restored);
    }

    // ─── Display 格式 ─────────────────────────────────

    #[test]
    fn test_timestamp_display_format() {
        let ts = Timestamp::from_nanos(0);
        let s = format!("{ts}");
        assert!(s.starts_with("1970-01-01T00:00:00"));
        assert!(s.ends_with('Z'));
    }
}
