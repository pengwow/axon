//! 三个内置工具的测试辅助模块

use serde_json::json;
use axon_llm::tools::{Tool, ToolError};

// ─── 市场分析工具 ─────────────────────────────────────────

pub struct AnalyzeMarketTool;

#[async_trait::async_trait]
impl Tool for AnalyzeMarketTool {
    fn name(&self) -> &'static str { "analyze_market" }
    fn description(&self) -> &'static str { "分析指定交易对的市场数据，返回价格、成交量、趋势等信息" }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "交易对符号，如 'BTC/USDT'"},
                "timeframe": {
                    "type": "string",
                    "enum": ["1m", "5m", "15m", "1h", "4h", "1d"],
                    "description": "K线时间周期"
                }
            },
            "required": ["symbol", "timeframe"]
        })
    }
    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let symbol = args["symbol"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("缺少 symbol 参数".into()))?;
        let timeframe = args["timeframe"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("缺少 timeframe 参数".into()))?;

        // 验证 timeframe
        if !["1m", "5m", "15m", "1h", "4h", "1d"].contains(&timeframe) {
            return Err(ToolError::InvalidArguments(format!("无效 timeframe: {}", timeframe)));
        }

        let result = json!({
            "symbol": symbol,
            "timeframe": timeframe,
            "price": 50000.0,
            "change_24h": 2.5,
            "volume_24h": 1234567890.0,
            "trend": "bullish",
            "rsi": 65.3,
            "macd_signal": "buy"
        });
        Ok(result.to_string())
    }
}

// ─── 投资组合工具 ─────────────────────────────────────────

pub struct CheckPortfolioTool;

#[async_trait::async_trait]
impl Tool for CheckPortfolioTool {
    fn name(&self) -> &'static str { "check_portfolio" }
    fn description(&self) -> &'static str { "查询当前投资组合状态，包括持仓、盈亏、净值" }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "include_history": {
                    "type": "boolean",
                    "description": "是否包含最近交易历史",
                    "default": false
                }
            }
        })
    }
    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let include_history = args["include_history"].as_bool().unwrap_or(false);

        let mut result = json!({
            "total_value": 100000.0,
            "cash": 50000.0,
            "positions": [
                {"symbol": "BTC/USDT", "quantity": 0.5, "avg_price": 48000.0, "current_price": 50000.0, "pnl": 1000.0},
                {"symbol": "ETH/USDT", "quantity": 10.0, "avg_price": 3000.0, "current_price": 3200.0, "pnl": 2000.0}
            ],
            "total_pnl": 3000.0,
            "daily_pnl": 500.0
        });

        if include_history {
            result["history"] = json!([
                {"action": "buy", "symbol": "BTC/USDT", "quantity": 0.5, "price": 48000.0},
                {"action": "buy", "symbol": "ETH/USDT", "quantity": 10.0, "price": 3000.0}
            ]);
        }

        Ok(result.to_string())
    }
}

// ─── 订单提交工具 ─────────────────────────────────────────

pub struct SubmitOrderTool;

#[async_trait::async_trait]
impl Tool for SubmitOrderTool {
    fn name(&self) -> &'static str { "submit_order" }
    fn description(&self) -> &'static str { "提交交易订单（仅支持限价单，市价单需用户确认）" }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "交易对符号"},
                "side": {"type": "string", "enum": ["buy", "sell"], "description": "交易方向"},
                "order_type": {"type": "string", "enum": ["market", "limit"], "description": "订单类型"},
                "quantity": {"type": "number", "description": "交易数量", "exclusiveMinimum": 0},
                "price": {"type": "number", "description": "限价单价格（市价单时忽略）"}
            },
            "required": ["symbol", "side", "order_type", "quantity"]
        })
    }
    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let _symbol = args["symbol"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("缺少 symbol".into()))?;
        let side = args["side"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("缺少 side".into()))?;
        let order_type = args["order_type"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("缺少 order_type".into()))?;
        let quantity = args["quantity"].as_f64()
            .ok_or_else(|| ToolError::InvalidArguments("缺少 quantity".into()))?;

        if !["buy", "sell"].contains(&side) {
            return Err(ToolError::InvalidArguments(format!("无效 side: {}", side)));
        }
        if !["market", "limit"].contains(&order_type) {
            return Err(ToolError::InvalidArguments(format!("无效 order_type: {}", order_type)));
        }
        if quantity <= 0.0 {
            return Err(ToolError::InvalidArguments("quantity 必须大于 0".into()));
        }

        // 市价单需要确认
        if order_type == "market" {
            return Err(ToolError::ExecutionFailed(
                "市价单需要用户确认，暂不支持自动提交".into()
            ));
        }

        let result = json!({
            "order_id": 12345,
            "symbol": _symbol,
            "side": side,
            "type": order_type,
            "quantity": quantity,
            "price": args["price"],
            "status": "submitted",
            "message": "订单已提交，等待撮合"
        });
        Ok(result.to_string())
    }
}
