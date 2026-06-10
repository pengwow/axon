//! 订单类型枚举

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::{Price, Quantity};

/// 订单类型
///
/// 描述订单的撮合行为：市价、限价、止损、止损限价、冰山等。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderType {
    /// 市价单 - 立即以最优价成交（默认）
    #[default]
    Market,
    /// 限价单 - 在指定价格或更优价格成交
    Limit {
        /// 限价
        price: Price,
    },
    /// 止损单 - 触发价后转为市价单
    Stop {
        /// 触发价
        trigger: Price,
    },
    /// 止损限价单 - 触发价后转为限价单
    StopLimit {
        /// 触发价
        trigger: Price,
        /// 限价
        price: Price,
    },
    /// 冰山单 - 仅显示部分数量
    Iceberg {
        /// 每次可见数量
        visible: Quantity,
        /// 隐藏总数量
        hidden: Quantity,
    },
}

impl OrderType {
    /// 是否需要指定价格
    #[inline]
    pub fn requires_price(&self) -> bool {
        matches!(self, Self::Limit { .. } | Self::StopLimit { .. })
    }

    /// 是否包含冰山属性
    #[inline]
    pub fn is_iceberg(&self) -> bool {
        matches!(self, Self::Iceberg { .. })
    }

    /// 是否为条件订单（需要触发）
    #[inline]
    pub fn is_conditional(&self) -> bool {
        matches!(self, Self::Stop { .. } | Self::StopLimit { .. })
    }

    /// 获取冰山单的可见数量（非冰山单返回 `None`）
    pub fn iceberg_visible(&self) -> Option<Quantity> {
        match self {
            Self::Iceberg { visible, .. } => Some(*visible),
            _ => None,
        }
    }

    /// 获取限价（非限价单返回 `None`）
    pub fn limit_price(&self) -> Option<Price> {
        match self {
            Self::Limit { price } | Self::StopLimit { price, .. } => Some(*price),
            _ => None,
        }
    }

    /// 获取触发价（非条件单返回 `None`）
    pub fn trigger_price(&self) -> Option<Price> {
        match self {
            Self::Stop { trigger } | Self::StopLimit { trigger, .. } => Some(*trigger),
            _ => None,
        }
    }
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Market => write!(f, "MARKET"),
            Self::Limit { price } => write!(f, "LIMIT@{}", price),
            Self::Stop { trigger } => write!(f, "STOP@{}", trigger),
            Self::StopLimit { trigger, price } => write!(f, "STOP_LIMIT@{}/{}", trigger, price),
            Self::Iceberg { visible, hidden } => {
                write!(f, "ICEBERG({}+{})", visible, hidden)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_type_default_is_market() {
        assert_eq!(OrderType::default(), OrderType::Market);
    }

    #[test]
    fn test_order_type_requires_price() {
        assert!(!OrderType::Market.requires_price());
        assert!(OrderType::Limit {
            price: Price::from_f64(100.0)
        }
        .requires_price());
        assert!(OrderType::StopLimit {
            trigger: Price::from_f64(101.0),
            price: Price::from_f64(100.0),
        }
        .requires_price());
        assert!(!OrderType::Stop {
            trigger: Price::from_f64(101.0)
        }
        .requires_price());
    }

    #[test]
    fn test_order_type_is_iceberg() {
        assert!(!OrderType::Market.is_iceberg());
        let iceberg = OrderType::Iceberg {
            visible: Quantity::from_f64(1.0),
            hidden: Quantity::from_f64(9.0),
        };
        assert!(iceberg.is_iceberg());
    }

    #[test]
    fn test_order_type_is_conditional() {
        assert!(!OrderType::Market.is_conditional());
        assert!(OrderType::Stop {
            trigger: Price::from_f64(100.0)
        }
        .is_conditional());
        assert!(OrderType::StopLimit {
            trigger: Price::from_f64(100.0),
            price: Price::from_f64(101.0),
        }
        .is_conditional());
    }

    #[test]
    fn test_order_type_iceberg_visible() {
        let iceberg = OrderType::Iceberg {
            visible: Quantity::from_f64(2.0),
            hidden: Quantity::from_f64(8.0),
        };
        assert_eq!(iceberg.iceberg_visible(), Some(Quantity::from_f64(2.0)));
        assert_eq!(OrderType::Market.iceberg_visible(), None);
    }

    #[test]
    fn test_order_type_limit_price() {
        let limit = OrderType::Limit {
            price: Price::from_f64(50.0),
        };
        assert_eq!(limit.limit_price(), Some(Price::from_f64(50.0)));

        let stop_limit = OrderType::StopLimit {
            trigger: Price::from_f64(55.0),
            price: Price::from_f64(50.0),
        };
        assert_eq!(stop_limit.limit_price(), Some(Price::from_f64(50.0)));

        assert_eq!(OrderType::Market.limit_price(), None);
    }

    #[test]
    fn test_order_type_trigger_price() {
        let stop = OrderType::Stop {
            trigger: Price::from_f64(110.0),
        };
        assert_eq!(stop.trigger_price(), Some(Price::from_f64(110.0)));

        let stop_limit = OrderType::StopLimit {
            trigger: Price::from_f64(110.0),
            price: Price::from_f64(109.0),
        };
        assert_eq!(stop_limit.trigger_price(), Some(Price::from_f64(110.0)));

        assert_eq!(OrderType::Market.trigger_price(), None);
    }

    #[test]
    fn test_order_type_display() {
        assert_eq!(format!("{}", OrderType::Market), "MARKET");
        assert_eq!(
            format!(
                "{}",
                OrderType::Limit {
                    price: Price::from_f64(100.0)
                }
            ),
            "LIMIT@100"
        );
        assert_eq!(
            format!(
                "{}",
                OrderType::Stop {
                    trigger: Price::from_f64(110.0)
                }
            ),
            "STOP@110"
        );
        assert_eq!(
            format!(
                "{}",
                OrderType::StopLimit {
                    trigger: Price::from_f64(110.0),
                    price: Price::from_f64(109.0),
                }
            ),
            "STOP_LIMIT@110/109"
        );
    }
}
