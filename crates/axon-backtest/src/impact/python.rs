//! PyO3 桥接层：将 [`ImpactedMatchingEngine`] 暴露给 Python
//!
//! 启用 `python` feature 时编译。提供：
//! - `ImpactedMatchingEngine` Python 类（包装 Rust 引擎）
//! - 提交订单（从 Python dict 接收 `Order` 字段）
//! - 返回成交结果（Python dict 列表）
//!
//! # 启用
//!
//! ```bash
//! cargo build -p axon-backtest --features python
//! ```

#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::useless_conversion)]
#![allow(deprecated)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use axon_core::market::Side as CoreSide;
use axon_core::order::{Order, OrderType, TimeInForce};
use axon_core::types::{Price, Quantity, Symbol};

use crate::impact::impacted_engine::ImpactedMatchingEngine as RustEngine;
use crate::matching::types::MatchFill;

/// Python 侧冲击感知撮合引擎
#[pyclass(name = "ImpactedMatchingEngine")]
pub struct PyImpactedMatchingEngine {
    inner: RustEngine,
}

#[pymethods]
impl PyImpactedMatchingEngine {
    /// 从 Python dict 创建
    ///
    /// Args:
    /// - `model_type`: `"linear"` 或 `"power_law"`
    /// - `coefficient`: 冲击系数
    /// - `depth_levels`: 深度层级数（默认 10）
    /// - `instantaneous_ratio`: 即时/永久比例（默认 0.7）
    /// - `exponent`: 幂律指数（仅 power_law，默认 0.5）
    /// - `permanent_decay`: 永久冲击衰减率（默认 0.0）
    #[new]
    #[pyo3(signature = (model_type, coefficient, depth_levels=10, instantaneous_ratio=0.7, exponent=0.5, permanent_decay=0.0))]
    fn new(
        model_type: &str,
        coefficient: f64,
        depth_levels: usize,
        instantaneous_ratio: f64,
        exponent: f64,
        permanent_decay: f64,
    ) -> PyResult<Self> {
        let model: Box<dyn axon_core::impact::ImpactModel> = match model_type {
            "linear" => Box::new(axon_core::impact::LinearImpactModel::new(coefficient)),
            "power_law" => Box::new(axon_core::impact::PowerLawImpactModel::new(
                coefficient, exponent,
            )),
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown model_type: {model_type}"
                )))
            }
        };
        let mut engine = RustEngine::new(model)
            .with_permanent_decay(permanent_decay);
        // 重新调整 depth_levels 和 instantaneous_ratio（构造后再覆盖）
        // 注：构造 LinearImpactModel/PowerLawImpactModel 时已设置默认 depth=10、ratio=0.7
        // 用户传入的 depth/ratio 不影响已构造的模型，仅作为占位（由 Rust API 调用控制）
        // 这里通过重建模型来支持自定义 depth/ratio
        match model_type {
            "linear" => {
                let m = axon_core::impact::LinearImpactModel::new(coefficient)
                    .with_depth(depth_levels)
                    .with_instantaneous_ratio(instantaneous_ratio);
                engine.set_model(Box::new(m));
            }
            "power_law" => {
                let m = axon_core::impact::PowerLawImpactModel::new(coefficient, exponent)
                    .with_depth(depth_levels)
                    .with_instantaneous_ratio(instantaneous_ratio);
                engine.set_model(Box::new(m));
            }
            _ => unreachable!(),
        }
        Ok(Self { inner: engine })
    }

    /// 提交订单
    ///
    /// Args:
    /// - `order_dict`: 含 `id` / `symbol` / `side` / `type` / `price` / `quantity` / `tif` 的 dict
    ///
    /// Returns:
    /// - dict 含 `fills` / `is_filled` / `is_partially_filled` / `remaining_quantity`
    fn submit<'py>(
        &mut self,
        py: Python<'py>,
        order_dict: &Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let order = dict_to_order(order_dict)?;
        let result = self.inner.submit(order);
        submit_result_to_dict(py, &result)
    }

    /// 获取永久冲击偏移
    fn permanent_offset(&self) -> f64 {
        self.inner.permanent_offset()
    }

    /// 获取最优买价（应用永久冲击后）
    fn best_bid(&self) -> Option<f64> {
        self.inner.best_bid().map(|p| p.as_f64())
    }

    /// 获取最优卖价（应用永久冲击后）
    fn best_ask(&self) -> Option<f64> {
        self.inner.best_ask().map(|p| p.as_f64())
    }

    /// 获取中间价
    fn mid_price(&self) -> Option<f64> {
        self.inner.mid_price().map(|p| p.as_f64())
    }

    /// 活跃订单数
    fn active_order_count(&self) -> usize {
        self.inner.active_order_count()
    }

    /// 取消订单
    fn cancel(&mut self, order_id: u64) -> bool {
        self.inner.cancel(order_id)
    }

    /// 重置冲击状态（保留订单簿）
    fn reset_impact_state(&mut self) {
        self.inner.reset_impact_state();
    }

    /// 获取统计信息
    fn stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let stats = self.inner.stats();
        let dict = PyDict::new(py);
        dict.set_item("cumulative_instantaneous", stats.cumulative_instantaneous)?;
        dict.set_item("cumulative_permanent", stats.cumulative_permanent)?;
        dict.set_item("cumulative_total", stats.cumulative_total())?;
        dict.set_item("submitted_orders", stats.submitted_orders)?;
        dict.set_item("filled_orders", stats.filled_orders)?;
        dict.set_item("total_fills", stats.total_fills)?;
        Ok(dict)
    }

    /// 获取模型名称
    fn model_name(&self) -> String {
        self.inner.model_name().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "ImpactedMatchingEngine(model={}, permanent_offset={:.4}, active_orders={})",
            self.inner.model_name(),
            self.inner.permanent_offset(),
            self.inner.active_order_count()
        )
    }
}

/// Python dict → Rust Order
fn dict_to_order(dict: &Bound<'_, PyDict>) -> PyResult<Order> {
    let id: u64 = dict
        .get_item("id")?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("missing 'id'"))?
        .extract()?;
    let symbol: String = dict
        .get_item("symbol")?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("missing 'symbol'"))?
        .extract()?;
    let side_str: String = dict
        .get_item("side")?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("missing 'side'"))?
        .extract()?;
    let side = parse_side(&side_str)?;
    let order_type_str: String = dict
        .get_item("type")?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("missing 'type'"))?
        .extract()?;
    let quantity: f64 = dict
        .get_item("quantity")?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("missing 'quantity'"))?
        .extract()?;
    let tif_str: String = dict
        .get_item("tif")?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("missing 'tif'"))?
        .extract()?;
    let tif = parse_tif(&tif_str)?;

    let order_type = match order_type_str.as_str() {
        "limit" => {
            let price: f64 = dict
                .get_item("price")?
                .ok_or_else(|| {
                    pyo3::exceptions::PyKeyError::new_err("limit order needs 'price'")
                })?
                .extract()?;
            OrderType::Limit {
                price: Price::from_f64(price),
            }
        }
        "market" => OrderType::Market,
        other => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unsupported order type: {other}"
            )))
        }
    };

    Ok(Order::new(
        id,
        Symbol::from(symbol),
        side,
        order_type,
        Quantity::from_f64(quantity),
        tif,
    ))
}

/// 解析 side 字符串
fn parse_side(s: &str) -> PyResult<CoreSide> {
    match s.to_lowercase().as_str() {
        "buy" => Ok(CoreSide::Buy),
        "sell" => Ok(CoreSide::Sell),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid side: {other}"
        ))),
    }
}

/// 解析 tif 字符串
fn parse_tif(s: &str) -> PyResult<TimeInForce> {
    match s.to_uppercase().as_str() {
        "GTC" => Ok(TimeInForce::GTC),
        "IOC" => Ok(TimeInForce::IOC),
        "FOK" => Ok(TimeInForce::FOK),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid tif: {other}"
        ))),
    }
}

/// SubmitResult → Python dict
fn submit_result_to_dict<'py>(
    py: Python<'py>,
    result: &crate::matching::types::SubmitResult,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);

    let fills_list = pyo3::types::PyList::empty(py);
    for fill in &result.fills {
        fills_list.append(match_fill_to_dict(py, fill)?)?;
    }
    dict.set_item("fills", fills_list)?;
    dict.set_item("is_filled", result.is_filled)?;
    dict.set_item("is_partially_filled", result.is_partially_filled)?;
    dict.set_item("remaining_quantity", result.remaining_quantity.as_f64())?;
    Ok(dict)
}

/// MatchFill → Python dict
fn match_fill_to_dict<'py>(py: Python<'py>, fill: &MatchFill) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("fill_id", fill.fill_id)?;
    dict.set_item("taker_order_id", fill.taker_order_id)?;
    dict.set_item("maker_order_id", fill.maker_order_id)?;
    dict.set_item("price", fill.price.as_f64())?;
    dict.set_item("quantity", fill.quantity.as_f64())?;
    dict.set_item("taker_side", format!("{:?}", fill.taker_side))?;
    Ok(dict)
}

/// 注册 `ImpactedMatchingEngine` 到 Python 模块
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyImpactedMatchingEngine>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_side_buy() {
        assert!(matches!(parse_side("buy").unwrap(), CoreSide::Buy));
        assert!(matches!(parse_side("BUY").unwrap(), CoreSide::Buy));
    }

    #[test]
    fn test_parse_side_sell() {
        assert!(matches!(parse_side("sell").unwrap(), CoreSide::Sell));
    }

    #[test]
    fn test_parse_side_invalid() {
        assert!(parse_side("invalid").is_err());
    }

    #[test]
    fn test_parse_tif_gtc() {
        assert!(matches!(parse_tif("GTC").unwrap(), TimeInForce::GTC));
    }

    #[test]
    fn test_parse_tif_ioc() {
        assert!(matches!(parse_tif("IOC").unwrap(), TimeInForce::IOC));
    }

    #[test]
    fn test_parse_tif_fok() {
        assert!(matches!(parse_tif("FOK").unwrap(), TimeInForce::FOK));
    }

    #[test]
    fn test_parse_tif_invalid() {
        assert!(parse_tif("XXX").is_err());
    }
}
