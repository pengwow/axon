//! axon-llm 与 axon-explain 的集成层
//!
//! 启用 `explain` feature 后可用。
//!
//! ## 模块组织
//!
//! - [`types`] — `DecisionRecord` / `ExplainMode`
//! - [`store`] — `ExplanationStore`（tokio RwLock + FIFO 淘汰）
//! - [`bridge`] — `ExplainerBridge`（spawn_blocking 包装同步 Explainer）
//! - [`recorder`] — `DecisionRecorder`（fire-and-forget 异步记录）
//! - [`tools`] — `QueryExplanationTool` / `ComputeExplanationTool`

#![cfg(feature = "explain")]

mod bridge;
mod recorder;
mod store;
mod tools;
mod types;

pub use bridge::ExplainerBridge;
pub use recorder::DecisionRecorder;
pub use store::{ExplanationStore, DEFAULT_CAPACITY};
pub use tools::{
    ComputeExplanationTool, QueryExplanationTool, DEFAULT_COMPUTE_TIMEOUT_MS,
    DEFAULT_QUERY_TIMEOUT_MS,
};
pub use types::{DecisionRecord, ExplainMode};
