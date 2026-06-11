//! 版本过滤器

use std::collections::HashMap;

use crate::types::{ModelStage, SemVer};

/// 版本查询过滤器
#[derive(Debug, Clone, Default)]
pub struct VersionFilter {
    /// 阶段过滤
    pub stage: Option<ModelStage>,
    /// 标签过滤（key -> value，全部匹配）
    pub tags: HashMap<String, String>,
    /// 最小版本（含）
    pub min_version: Option<SemVer>,
    /// 最大版本（含）
    pub max_version: Option<SemVer>,
    /// 限制返回数量
    pub limit: Option<usize>,
}

impl VersionFilter {
    /// 创建新过滤器
    pub fn new() -> Self {
        Self::default()
    }

    /// 限定阶段
    pub fn with_stage(mut self, stage: ModelStage) -> Self {
        self.stage = Some(stage);
        self
    }

    /// 限定标签
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// 限定最小版本
    pub fn with_min_version(mut self, v: SemVer) -> Self {
        self.min_version = Some(v);
        self
    }

    /// 限定最大版本
    pub fn with_max_version(mut self, v: SemVer) -> Self {
        self.max_version = Some(v);
        self
    }

    /// 限制返回数量
    pub fn with_limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_builder() {
        let f = VersionFilter::new()
            .with_stage(ModelStage::Production)
            .with_tag("env", "prod")
            .with_limit(10);
        assert_eq!(f.stage, Some(ModelStage::Production));
        assert_eq!(f.tags.get("env").map(String::as_str), Some("prod"));
        assert_eq!(f.limit, Some(10));
    }
}
