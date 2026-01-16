use crate::{OrderId, Price, Quantity};

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct TradeInfo {
    order_id: OrderId,
    price: Price,
    quantity: Quantity,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct Trade {
    pub bid_trade: TradeInfo,
    pub ask_trade: TradeInfo,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Trades<'a>(Vec<&'a Trade>);
