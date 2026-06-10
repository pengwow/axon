//! 订单状态机与拒绝原因

use std::fmt;

use serde::{Deserialize, Serialize};

/// 订单状态
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderStatus {
    /// 已创建，等待提交
    #[default]
    Created,
    /// 已提交，等待撮合
    Pending,
    /// 部分成交
    PartiallyFilled,
    /// 完全成交
    Filled,
    /// 已取消
    Cancelled,
    /// 已拒绝
    Rejected,
    /// 已过期
    Expired,
}

impl OrderStatus {
    /// 状态转换是否合法
    pub fn can_transition_to(self, target: OrderStatus) -> bool {
        matches!(
            (self, target),
            (Self::Created, Self::Pending)
                | (Self::Created, Self::Rejected)
                | (Self::Pending, Self::PartiallyFilled)
                | (Self::Pending, Self::Filled)
                | (Self::Pending, Self::Cancelled)
                | (Self::Pending, Self::Rejected)
                | (Self::PartiallyFilled, Self::Filled)
                | (Self::PartiallyFilled, Self::Cancelled)
                | (Self::PartiallyFilled, Self::Expired)
        )
    }

    /// 是否为活跃状态（可继续撮合）
    #[inline]
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Created | Self::Pending | Self::PartiallyFilled)
    }

    /// 是否为终态
    #[inline]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Filled | Self::Cancelled | Self::Rejected | Self::Expired
        )
    }
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "CREATED"),
            Self::Pending => write!(f, "PENDING"),
            Self::PartiallyFilled => write!(f, "PARTIAL"),
            Self::Filled => write!(f, "FILLED"),
            Self::Cancelled => write!(f, "CANCELLED"),
            Self::Rejected => write!(f, "REJECTED"),
            Self::Expired => write!(f, "EXPIRED"),
        }
    }
}

/// 订单拒绝原因
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RejectReason {
    /// 资金不足
    InsufficientFunds,
    /// 保证金不足
    InsufficientMargin,
    /// 价格超出允许范围
    PriceOutOfRange,
    /// 数量过小（低于最小变动单位）
    QuantityTooSmall,
    /// 交易品种不存在
    SymbolNotFound,
    /// 市场休市
    MarketClosed,
    /// 触及风控限额
    RiskLimitExceeded,
    /// 其他原因
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_default_is_created() {
        assert_eq!(OrderStatus::default(), OrderStatus::Created);
    }

    #[test]
    fn test_status_can_transition_to() {
        // 合法转换
        assert!(OrderStatus::Created.can_transition_to(OrderStatus::Pending));
        assert!(OrderStatus::Created.can_transition_to(OrderStatus::Rejected));
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::PartiallyFilled));
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::Filled));
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::Cancelled));
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::Rejected));
        assert!(OrderStatus::PartiallyFilled.can_transition_to(OrderStatus::Filled));
        assert!(OrderStatus::PartiallyFilled.can_transition_to(OrderStatus::Cancelled));
        assert!(OrderStatus::PartiallyFilled.can_transition_to(OrderStatus::Expired));

        // 非法转换
        assert!(!OrderStatus::Filled.can_transition_to(OrderStatus::Pending));
        assert!(!OrderStatus::Cancelled.can_transition_to(OrderStatus::Filled));
        assert!(!OrderStatus::Rejected.can_transition_to(OrderStatus::Pending));
        assert!(!OrderStatus::Expired.can_transition_to(OrderStatus::Filled));
    }

    #[test]
    fn test_status_is_active() {
        assert!(OrderStatus::Created.is_active());
        assert!(OrderStatus::Pending.is_active());
        assert!(OrderStatus::PartiallyFilled.is_active());
        assert!(!OrderStatus::Filled.is_active());
        assert!(!OrderStatus::Cancelled.is_active());
        assert!(!OrderStatus::Rejected.is_active());
        assert!(!OrderStatus::Expired.is_active());
    }

    #[test]
    fn test_status_is_terminal() {
        assert!(!OrderStatus::Created.is_terminal());
        assert!(!OrderStatus::Pending.is_terminal());
        assert!(!OrderStatus::PartiallyFilled.is_terminal());
        assert!(OrderStatus::Filled.is_terminal());
        assert!(OrderStatus::Cancelled.is_terminal());
        assert!(OrderStatus::Rejected.is_terminal());
        assert!(OrderStatus::Expired.is_terminal());
    }

    #[test]
    fn test_status_display() {
        assert_eq!(format!("{}", OrderStatus::Created), "CREATED");
        assert_eq!(format!("{}", OrderStatus::Pending), "PENDING");
        assert_eq!(format!("{}", OrderStatus::PartiallyFilled), "PARTIAL");
        assert_eq!(format!("{}", OrderStatus::Filled), "FILLED");
        assert_eq!(format!("{}", OrderStatus::Cancelled), "CANCELLED");
        assert_eq!(format!("{}", OrderStatus::Rejected), "REJECTED");
        assert_eq!(format!("{}", OrderStatus::Expired), "EXPIRED");
    }
}
