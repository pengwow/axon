//! TDD 第八轮：三个内置工具

mod common_tools;
use axon_llm::tools::{Tool, ToolError};
use common_tools::{AnalyzeMarketTool, CheckPortfolioTool, SubmitOrderTool};

// ─── AnalyzeMarketTool ─────────────────────────────────────

#[tokio::test]
async fn test_analyze_market_returns_json() {
    let tool = AnalyzeMarketTool;
    let result = tool
        .execute(r#"{"symbol":"BTC/USDT","timeframe":"1h"}"#)
        .await
        .unwrap();

    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(v["symbol"], "BTC/USDT");
    assert_eq!(v["timeframe"], "1h");
    assert!(v["price"].is_number());
    assert!(v["change_24h"].is_number());
    assert!(v["volume_24h"].is_number());
    assert!(v["rsi"].is_number());
}

#[tokio::test]
async fn test_analyze_market_rejects_invalid_timeframe() {
    let tool = AnalyzeMarketTool;
    let err = tool
        .execute(r#"{"symbol":"BTC/USDT","timeframe":"invalid"}"#)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments(_)));
}

#[tokio::test]
async fn test_analyze_market_rejects_missing_symbol() {
    let tool = AnalyzeMarketTool;
    let err = tool.execute(r#"{"timeframe":"1h"}"#).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments(_)));
}

// ─── CheckPortfolioTool ─────────────────────────────────────

#[tokio::test]
async fn test_check_portfolio_returns_positions() {
    let tool = CheckPortfolioTool;
    let result = tool.execute(r#"{}"#).await.unwrap();

    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(v["total_value"].is_number());
    assert!(v["cash"].is_number());
    assert!(v["positions"].is_array());
    assert!(v["total_pnl"].is_number());
}

#[tokio::test]
async fn test_check_portfolio_with_history() {
    let tool = CheckPortfolioTool;
    let result = tool.execute(r#"{"include_history":true}"#).await.unwrap();

    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(
        v.get("history").is_some(),
        "include_history=true 应包含历史"
    );
}

// ─── SubmitOrderTool ────────────────────────────────────────

#[tokio::test]
async fn test_submit_order_validates_side() {
    let tool = SubmitOrderTool;
    let err = tool
        .execute(r#"{"symbol":"BTC/USDT","side":"invalid","order_type":"limit","quantity":0.1}"#)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments(_)));
}

#[tokio::test]
async fn test_submit_order_rejects_zero_quantity() {
    let tool = SubmitOrderTool;
    let err = tool
        .execute(r#"{"symbol":"BTC/USDT","side":"buy","order_type":"limit","quantity":0}"#)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments(_)));
}

#[tokio::test]
async fn test_submit_order_blocks_market_order() {
    let tool = SubmitOrderTool;
    let err = tool
        .execute(r#"{"symbol":"BTC/USDT","side":"buy","order_type":"market","quantity":0.1}"#)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::ExecutionFailed(_)));
}

#[tokio::test]
async fn test_submit_order_accepts_limit_order() {
    let tool = SubmitOrderTool;
    let result = tool
        .execute(r#"{"symbol":"BTC/USDT","side":"buy","order_type":"limit","quantity":0.1,"price":50000}"#)
        .await
        .unwrap();

    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(v["symbol"], "BTC/USDT");
    assert_eq!(v["status"], "submitted");
    assert!(v["order_id"].is_number());
}
