//! 模型签名（输入输出规范）

use serde::{Deserialize, Serialize};

/// 数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    /// 32 位浮点
    Float32,
    /// 64 位浮点
    Float64,
    /// 32 位整数
    Int32,
    /// 64 位整数
    Int64,
    /// 布尔
    Bool,
    /// 字符串
    String,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::Float32 => write!(f, "float32"),
            DataType::Float64 => write!(f, "float64"),
            DataType::Int32 => write!(f, "int32"),
            DataType::Int64 => write!(f, "int64"),
            DataType::Bool => write!(f, "bool"),
            DataType::String => write!(f, "string"),
        }
    }
}

/// 签名字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureField {
    /// 字段名
    pub name: String,
    /// 数据类型
    pub dtype: DataType,
    /// 形状（维度序列）
    pub shape: Vec<usize>,
    /// 描述
    pub description: Option<String>,
}

impl SignatureField {
    /// 创建新字段
    pub fn new(name: impl Into<String>, dtype: DataType, shape: Vec<usize>) -> Self {
        Self {
            name: name.into(),
            dtype,
            shape,
            description: None,
        }
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// 模型签名（输入输出规范）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSignature {
    /// 输入字段
    pub inputs: Vec<SignatureField>,
    /// 输出字段
    pub outputs: Vec<SignatureField>,
}

impl ModelSignature {
    /// 创建新签名
    pub fn new(inputs: Vec<SignatureField>, outputs: Vec<SignatureField>) -> Self {
        Self { inputs, outputs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_display() {
        assert_eq!(DataType::Float32.to_string(), "float32");
        assert_eq!(DataType::Int64.to_string(), "int64");
    }

    #[test]
    fn test_signature_field_builder() {
        let f = SignatureField::new("features", DataType::Float32, vec![128])
            .with_description("market features");
        assert_eq!(f.name, "features");
        assert_eq!(f.shape, vec![128]);
        assert_eq!(f.description.as_deref(), Some("market features"));
    }

    #[test]
    fn test_signature_serialize_roundtrip() {
        let sig = ModelSignature::new(
            vec![SignatureField::new("obs", DataType::Float32, vec![10])],
            vec![SignatureField::new("action", DataType::Float32, vec![3])],
        );
        let json = serde_json::to_string(&sig).unwrap();
        let back: ModelSignature = serde_json::from_str(&json).unwrap();
        assert_eq!(back.inputs.len(), 1);
        assert_eq!(back.outputs.len(), 1);
    }
}
