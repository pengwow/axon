//! AXON 实验追踪
//!
//! 统一 trait + 4 个后端（Memory / Local / MLflow / WandB）。

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod config;
pub mod error;
pub mod retry;
pub mod tracker;
pub mod types;

pub mod backends;

#[cfg(feature = "python")]
pub mod python;

pub use config::{MetricBuffer, TrackerBackend};
pub use error::{TrackerError, TrackerResult};
pub use retry::RetryPolicy;
pub use tracker::{ExperimentTracker, ExperimentTrackerExt};
pub use types::{
    ArtifactInfo, ExperimentConfig, ExperimentId, ImageFormat, MetricEntry, MetricValue,
    ParamValue, RunContext, RunId, RunStatus,
};

pub use backends::{LocalTracker, MemoryTracker};

#[cfg(feature = "http")]
pub use backends::{MlflowTracker, WandbTracker};
