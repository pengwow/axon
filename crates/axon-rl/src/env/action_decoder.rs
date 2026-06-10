//! 动作解码：RL 动作 → 订单

use crate::action::converter::{
    ActionConverter, ContinuousActionConverter, DiscreteActionConverter, Order, OrderType,
};
use crate::action::state::PortfolioState;
use crate::action::types::{Action, ActionSpace};
use crate::env::error::{EnvError, EnvResult};

/// 动作解码器：包装 `ActionConverter` 并处理环境特定的状态查询
pub struct ActionDecoder {
    /// 内部动作转换器
    converter: Box<dyn ActionConverter>,
}

impl ActionDecoder {
    /// 从离散动作空间构造解码器
    pub fn new_discrete(space: crate::action::types::DiscreteActionSpace, symbol: &str) -> Self {
        let converter = DiscreteActionConverter::new(space, symbol.to_string(), OrderType::Market);
        Self {
            converter: Box::new(converter),
        }
    }

    /// 从连续动作空间构造解码器
    pub fn new_continuous(
        space: crate::action::types::ContinuousActionSpace,
        symbol: &str,
    ) -> Self {
        let converter =
            ContinuousActionConverter::new(space, symbol.to_string(), OrderType::Market, 0.01);
        Self {
            converter: Box::new(converter),
        }
    }

    /// 从统一的 `ActionSpace` 构造解码器
    pub fn from_space(space: &ActionSpace, symbol: &str) -> EnvResult<Self> {
        match space {
            ActionSpace::Discrete(d) => Ok(Self::new_discrete(d.clone(), symbol)),
            ActionSpace::Continuous(c) => Ok(Self::new_continuous(c.clone(), symbol)),
        }
    }

    /// 动作 → 订单
    pub fn decode(&self, action: &Action, state: &PortfolioState) -> EnvResult<Option<Order>> {
        self.converter
            .to_order(action, state)
            .map_err(|e| EnvError::ActionError(e.to_string()))
    }

    /// 获取当前动作掩码
    pub fn mask(&self, state: &PortfolioState) -> Vec<bool> {
        self.converter.mask(state)
    }

    /// 获取动作空间引用
    pub fn action_space(&self) -> &ActionSpace {
        self.converter.action_space()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::converter::OrderSide;
    use crate::action::state::PortfolioState;
    use crate::action::types::{Action, DiscreteActionSpace, TradingDirection};

    fn empty_state() -> PortfolioState {
        PortfolioState {
            portfolio_value: 100_000.0,
            cash: 100_000.0,
            last_price: 5_000.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_discrete_decoder_hold_returns_none() {
        let decoder = ActionDecoder::new_discrete(
            DiscreteActionSpace::new(3, TradingDirection::LongOnly),
            "BTCUSDT",
        );
        let action = Action::discrete(0); // Hold
        let order = decoder.decode(&action, &empty_state()).unwrap();
        assert!(order.is_none());
    }

    #[test]
    fn test_discrete_decoder_buy_returns_order() {
        let decoder = ActionDecoder::new_discrete(
            DiscreteActionSpace::new(3, TradingDirection::LongOnly),
            "BTCUSDT",
        );
        let action = Action::discrete(1); // Buy bin 1
        let order = decoder.decode(&action, &empty_state()).unwrap().unwrap();
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.symbol, "BTCUSDT");
        assert!(order.quantity > 0.0);
    }

    #[test]
    fn test_continuous_decoder_returns_order() {
        let decoder = ActionDecoder::new_continuous(
            crate::action::types::ContinuousActionSpace::new(-1.0, 1.0),
            "BTCUSDT",
        );
        let action = Action::continuous(vec![0.5]);
        let order = decoder.decode(&action, &empty_state()).unwrap().unwrap();
        assert_eq!(order.side, OrderSide::Buy);
    }
}
