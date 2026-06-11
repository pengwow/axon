//! 提示模板系统
//!
//! 提供结构化提示模板，支持变量替换和 few-shot 示例注入。

use std::collections::HashMap;

/// 提示模板
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    /// 系统提示
    pub system: String,
    /// 用户提示（带变量占位符 `{var}`）
    pub user: String,
    /// Few-shot 示例（问题 → 回答）
    pub few_shot: Vec<(String, String)>,
}

impl PromptTemplate {
    /// 渲染用户提示中的变量
    pub fn render_user(&self, vars: &HashMap<&str, &str>) -> String {
        Self::replace_variables(&self.user, vars)
    }

    /// 渲染系统提示中的变量
    pub fn render_system(&self, vars: &HashMap<&str, &str>) -> String {
        Self::replace_variables(&self.system, vars)
    }

    /// 替换模板中的 `{key}` 变量
    fn replace_variables(template: &str, vars: &HashMap<&str, &str>) -> String {
        let mut result = template.to_string();
        for (key, value) in vars {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// 将 few-shot 示例追加到系统提示
    ///
    /// 格式：
    /// ```text
    /// ## 示例
    /// 用户: Q1
    /// 助手: A1
    /// 用户: Q2
    /// 助手: A2
    /// ```
    pub fn system_with_few_shot(&self) -> String {
        if self.few_shot.is_empty() {
            return self.system.clone();
        }

        let mut examples = String::from("\n\n## 示例\n");
        for (q, a) in &self.few_shot {
            use std::fmt::Write;
            let _ = writeln!(&mut examples, "用户: {}", q);
            let _ = writeln!(&mut examples, "助手: {}", a);
        }

        format!("{}{}", self.system, examples)
    }

    /// 交易分析师提示模板
    pub fn trader_system() -> Self {
        Self {
            system: r#"你是一个专业的量化交易分析师。回答用户的交易相关问题。
分析框架：
1. 趋势分析：识别主要趋势方向
2. 技术指标：RSI、MACD、布林带等
3. 成交量分析：量价关系
4. 风险评估：潜在风险因素

重要规则：
- 不要编造数据，使用工具获取真实数据
- 答案必须基于真实数据，不确定时明确说明
- 给出置信度和风险等级"#.to_string(),
            user: "分析 {symbol} 在 {timeframe} 时间周期的市场状况".to_string(),
            few_shot: vec![],
        }
    }

    /// 市场分析模板（包含 few-shot）
    pub fn market_analysis() -> Self {
        Self {
            system: r#"你是一个专业的量化交易分析师。
分析维度：趋势、技术指标、成交量、风险评估
输出格式：
- 市场状态：{bullish/bearish/sideways}
- 关键价位
- 建议操作：{buy/sell/hold}
- 置信度：{0-100}%"#.to_string(),
            user: "分析 {symbol} 在 {timeframe}".to_string(),
            few_shot: vec![
                (
                    "分析 BTC/USDT 在 1h".to_string(),
                    "市场状态：Bullish\n关键价位：$48,500 支撑 / $51,200 阻力\n建议操作：Buy\n置信度：75%\n风险等级：Medium".to_string(),
                ),
            ],
        }
    }
}
