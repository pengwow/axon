//! # axon-monitor
//!
//! 监控告警系统：指标采集、延迟直方图、告警规则、健康检查。
//!
//! ## 核心功能
//!
//! - **指标采集**：Counter（计数器）、Gauge（仪表）、Histogram（直方图）
//! - **原子操作**：所有指标使用原子操作，无锁高性能
//! - **告警规则**：阈值告警、趋势告警、缺失告警
//! - **健康检查**：组件健康状态、Kubernetes 探针接口
//! - **Prometheus 导出**：标准 Prometheus 格式输出
//!
//! ## 使用示例
//!
//! ```rust
//! use axon_monitor::{MetricsRegistry, AlertRule, AlertSeverity, ThresholdCondition};
//!
//! // 创建指标注册中心
//! let mut registry = MetricsRegistry::new();
//!
//! // 注册指标
//! let counter = registry.register_counter("orders_total");
//! let gauge = registry.register_gauge("daily_pnl");
//! let histogram = registry.register_histogram("order_latency_ns");
//!
//! // 记录指标
//! counter.inc();
//! gauge.set(-1000.0);
//! histogram.observe(150_000.0); // 150µs
//!
//! // 添加告警规则
//! registry.add_alert_rule(AlertRule::Threshold {
//!     metric_name: "order_latency_ns".into(),
//!     condition: ThresholdCondition::GreaterThan(10_000_000.0), // > 10ms
//!     severity: AlertSeverity::Warning,
//!     message: "order latency exceeds 10ms".into(),
//! });
//!
//! // 检查告警
//! registry.check_alerts("order_latency_ns", 50_000_000.0);
//! let alerts = registry.get_alerts();
//! ```
//!
//! ## 性能
//!
//! | 操作 | 延迟 |
//! |------|------|
//! | Counter inc | 1.6ns |
//! | Gauge set | 464ps |
//! | Histogram observe | 6.5ns |
//! | Histogram quantile | 3ns |
//! | Alert check | 4.8ns |

pub mod alert;
pub mod error;
pub mod health;
pub mod metrics;
pub mod registry;

pub use alert::{AlertEvent, AlertRule, AlertSeverity, ThresholdCondition};
pub use error::MonitorError;
pub use health::{ComponentHealth, HealthCheck, HealthService};
pub use metrics::{AtomicCounter, AtomicGauge, LatencyHistogram, LatencyPercentiles};
pub use registry::MetricsRegistry;
