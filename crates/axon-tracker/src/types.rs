//! 实验追踪数据类型

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// 全局唯一实验 ID
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExperimentId(pub String);

/// 运行实例 ID
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunId(pub String);

/// 图像格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    /// PNG
    Png,
    /// JPEG
    Jpeg,
    /// SVG
    Svg,
}

/// 指标值（标量/直方图/图像/表格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    /// 标量
    Scalar(f64),
    /// 直方图
    Histogram {
        /// 原始值
        values: Vec<f64>,
        /// 分箱边界
        bins: Vec<f64>,
    },
    /// 图像
    Image {
        /// 字节数据
        data: Vec<u8>,
        /// 格式
        format: ImageFormat,
        /// 宽度（像素）
        width: u32,
        /// 高度（像素）
        height: u32,
    },
    /// 表格
    Table {
        /// 列名
        columns: Vec<String>,
        /// 行数据
        rows: Vec<Vec<String>>,
    },
}

/// 参数值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamValue {
    /// 整数
    Int(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 布尔
    Bool(bool),
    /// 列表
    List(Vec<ParamValue>),
}

impl std::fmt::Display for ParamValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::List(v) => {
                let strs: Vec<_> = v.iter().map(|p| p.to_string()).collect();
                write!(f, "[{}]", strs.join(", "))
            }
        }
    }
}

/// 已记录的指标条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    /// 指标名
    pub key: String,
    /// 指标值
    pub value: MetricValue,
    /// 训练步数
    pub step: usize,
    /// 时间戳
    pub timestamp: SystemTime,
}

/// 运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 被终止
    Killed,
}

impl RunStatus {
    /// 返回 MLflow 状态字符串
    pub fn as_mlflow_str(&self) -> &'static str {
        match self {
            Self::Running => "RUNNING",
            Self::Completed => "FINISHED",
            Self::Failed => "FAILED",
            Self::Killed => "KILLED",
        }
    }
}

/// 实验配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExperimentConfig {
    /// 超参数
    pub hyperparameters: HashMap<String, ParamValue>,
    /// Git commit hash
    pub git_commit: Option<String>,
    /// 数据集 hash
    pub dataset_hash: Option<String>,
    /// 随机种子
    pub seed: Option<u64>,
    /// 开始时间
    pub start_time: SystemTime,
    /// 标签
    pub tags: HashMap<String, String>,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            hyperparameters: HashMap::new(),
            git_commit: None,
            dataset_hash: None,
            seed: None,
            start_time: SystemTime::now(),
            tags: HashMap::new(),
        }
    }
}

/// 产物元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    /// 名称
    pub name: String,
    /// 路径
    pub path: PathBuf,
    /// 字节数
    pub size_bytes: u64,
    /// 内容 hash
    pub content_hash: String,
    /// 时间戳
    pub timestamp: SystemTime,
}

/// 运行上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunContext {
    /// run ID
    pub run_id: RunId,
    /// experiment ID
    pub experiment_id: ExperimentId,
    /// 配置
    pub config: ExperimentConfig,
    /// 状态
    pub status: RunStatus,
    /// 开始时间
    pub start_time: SystemTime,
    /// 结束时间
    pub end_time: Option<SystemTime>,
    /// 指标历史
    pub metrics: Vec<MetricEntry>,
    /// 产物列表
    pub artifacts: Vec<ArtifactInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_value_display() {
        assert_eq!(ParamValue::Int(42).to_string(), "42");
        #[allow(clippy::approx_constant)]
        let pi = 3.14;
        assert_eq!(ParamValue::Float(pi).to_string(), "3.14");
        assert_eq!(ParamValue::String("ppo".into()).to_string(), "ppo");
        assert_eq!(ParamValue::Bool(true).to_string(), "true");
        assert_eq!(
            ParamValue::List(vec![ParamValue::Int(1), ParamValue::Int(2)]).to_string(),
            "[1, 2]"
        );
    }

    #[test]
    fn test_run_status_mlflow_str() {
        assert_eq!(RunStatus::Running.as_mlflow_str(), "RUNNING");
        assert_eq!(RunStatus::Completed.as_mlflow_str(), "FINISHED");
        assert_eq!(RunStatus::Failed.as_mlflow_str(), "FAILED");
        assert_eq!(RunStatus::Killed.as_mlflow_str(), "KILLED");
    }

    #[test]
    fn test_experiment_config_default() {
        let cfg = ExperimentConfig::default();
        assert!(cfg.hyperparameters.is_empty());
        assert!(cfg.git_commit.is_none());
    }

    #[test]
    fn test_metric_value_serialize_scalar() {
        let v = MetricValue::Scalar(1.5);
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("1.5"));
    }

    #[test]
    fn test_metric_entry_serialize() {
        let entry = MetricEntry {
            key: "loss".to_string(),
            value: MetricValue::Scalar(0.5),
            step: 100,
            timestamp: SystemTime::UNIX_EPOCH,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("loss"));
        assert!(json.contains("100"));
    }
}
