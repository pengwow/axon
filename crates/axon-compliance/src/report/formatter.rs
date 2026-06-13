//! 报告格式化导出器
//!
//! 支持 JSON 和 CSV 格式导出。

use serde::Serialize;

use crate::error::{ComplianceError, ComplianceResult};
use crate::report::ReportFormat;

/// 报告导出器
pub struct ReportExporter;

impl ReportExporter {
    /// 导出报告为指定格式
    pub fn export<T: Serialize>(report: &T, format: ReportFormat) -> ComplianceResult<Vec<u8>> {
        match format {
            ReportFormat::JSON => Self::export_json(report),
            ReportFormat::CSV => Self::export_csv(report),
        }
    }

    /// 导出为 JSON
    fn export_json<T: Serialize>(report: &T) -> ComplianceResult<Vec<u8>> {
        let json = serde_json::to_string_pretty(report)
            .map_err(|e| ComplianceError::SerializationError(e.to_string()))?;
        Ok(json.into_bytes())
    }

    /// 导出为 CSV（扁平结构，单行数据）
    fn export_csv<T: Serialize>(report: &T) -> ComplianceResult<Vec<u8>> {
        let value: serde_json::Value = serde_json::to_value(report)
            .map_err(|e| ComplianceError::SerializationError(e.to_string()))?;

        let mut wtr = csv::Writer::from_writer(vec![]);

        if let Some(obj) = value.as_object() {
            // 写入表头
            let headers: Vec<&String> = obj.keys().collect();
            wtr.write_record(headers.iter().map(|h| h.as_str()))
                .map_err(|e| ComplianceError::ReportError(e.to_string()))?;

            // 写入数据行（扁平化嵌套值）
            let values: Vec<String> = obj
                .values()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect();
            wtr.write_record(&values)
                .map_err(|e| ComplianceError::ReportError(e.to_string()))?;
        }

        wtr.flush()
            .map_err(|e| ComplianceError::ReportError(e.to_string()))?;

        let data = wtr
            .into_inner()
            .map_err(|e| ComplianceError::ReportError(e.to_string()))?;

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{DailyReport, FeeBreakdown};
    use chrono::NaiveDate;

    fn make_test_report() -> DailyReport {
        DailyReport {
            date: NaiveDate::from_ymd_opt(2026, 6, 13).unwrap(),
            account_id: "test".into(),
            starting_balance: 100000.0,
            ending_balance: 100500.0,
            net_pnl: 500.0,
            realized_pnl: 500.0,
            unrealized_pnl: 0.0,
            total_fees: 50.0,
            total_trades: 10,
            winning_trades: 6,
            losing_trades: 4,
            largest_win: 200.0,
            largest_loss: -100.0,
            positions: vec![],
            fee_breakdown: FeeBreakdown {
                trading_fees: 50.0,
                funding_fees: 0.0,
                withdrawal_fees: 0.0,
                other_fees: 0.0,
                total: 50.0,
                fee_currency: "USDT".into(),
            },
            trade_count: 10,
        }
    }

    #[test]
    fn test_json_export() {
        let report = make_test_report();
        let data = ReportExporter::export(&report, ReportFormat::JSON).unwrap();
        let json_str = String::from_utf8(data).unwrap();

        // 验证是有效 JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
        assert_eq!(parsed["account_id"], "test");
        assert_eq!(parsed["net_pnl"], 500.0);
    }

    #[test]
    fn test_csv_export() {
        let report = make_test_report();
        let data = ReportExporter::export(&report, ReportFormat::CSV).unwrap();
        let csv_str = String::from_utf8(data).unwrap();

        // CSV 应包含表头和数据行
        let lines: Vec<&str> = csv_str.trim().lines().collect();
        assert!(lines.len() >= 2); // 至少表头 + 1 行数据
    }

    #[test]
    fn test_json_roundtrip() {
        let report = make_test_report();
        let data = ReportExporter::export(&report, ReportFormat::JSON).unwrap();
        let json_str = String::from_utf8(data).unwrap();
        let deserialized: DailyReport = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.date, report.date);
        assert_eq!(deserialized.account_id, report.account_id);
        assert!((deserialized.net_pnl - report.net_pnl).abs() < f64::EPSILON);
    }
}
