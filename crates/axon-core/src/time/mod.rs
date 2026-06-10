//! 时间相关类型
//!
//! 提供纳秒级精度时间戳、单调时钟与时间精度枚举。
//! 设计详见 [`axon-design/01-tdd/01-phase1-core/01-timestamp.md`](../../../../axon-design/01-tdd/01-phase1-core/01-timestamp.md)。

pub mod monotonic;
pub mod precision;
pub mod timestamp;

pub use monotonic::MonotonicClock;
pub use precision::TimePrecision;
pub use timestamp::{Timestamp, TimestampError, TimestampResult};
