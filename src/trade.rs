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

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Trades(Vec<Trade>);

impl Trades {
    pub fn new() -> Trades {
        Trades(Vec::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn push(&mut self, trade: Trade) {
        self.0.push(trade)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Trade> {
        self.0.iter()
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    pub fn last(&self) -> Option<&Trade> {
        self.0.last()
    }
}

impl TradeInfo {
    pub fn new(order_id: OrderId, price: Price, quantity: Quantity) -> TradeInfo {
        TradeInfo {
            order_id,
            price,
            quantity,
        }
    }
    pub fn price(&self) -> Price {
        self.price
    }
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }
    pub fn order_id(&self) -> OrderId {
        self.order_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OrderId, Price, Quantity};

    fn order_id(id: u64) -> OrderId {
        OrderId::new(id)
    }

    fn price(p: u32) -> Price {
        Price::new(p)
    }

    fn qty(q: u32) -> Quantity {
        Quantity(q)
    }

    fn sample_trade(bid_id: u64, ask_id: u64, p: u32, q: u32) -> Trade {
        Trade {
            bid_trade: TradeInfo {
                order_id: order_id(bid_id),
                price: price(p),
                quantity: qty(q),
            },
            ask_trade: TradeInfo {
                order_id: order_id(ask_id),
                price: price(p),
                quantity: qty(q),
            },
        }
    }

    #[test]
    fn new_trades_is_empty() {
        let trades = Trades::new();
        assert!(trades.is_empty());
        assert_eq!(trades.len(), 0);
    }

    #[test]
    fn push_adds_trade() {
        let mut trades = Trades::new();
        trades.push(sample_trade(1, 2, 100, 50));
        assert_eq!(trades.len(), 1);
        assert!(!trades.is_empty());
    }

    #[test]
    fn push_multiple_trades() {
        let mut trades = Trades::new();
        trades.push(sample_trade(1, 2, 100, 50));
        trades.push(sample_trade(3, 4, 101, 30));
        trades.push(sample_trade(5, 6, 99, 20));
        assert_eq!(trades.len(), 3);
    }

    #[test]
    fn iter_yields_all_trades() {
        let mut trades = Trades::new();
        trades.push(sample_trade(1, 2, 100, 50));
        trades.push(sample_trade(3, 4, 101, 30));

        let collected: Vec<_> = trades.iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn clear_removes_all_trades() {
        let mut trades = Trades::new();
        trades.push(sample_trade(1, 2, 100, 50));
        trades.push(sample_trade(3, 4, 101, 30));
        trades.clear();
        assert!(trades.is_empty());
    }

    #[test]
    fn last_returns_most_recent_trade() {
        let mut trades = Trades::new();
        trades.push(sample_trade(1, 2, 100, 50));
        trades.push(sample_trade(3, 4, 101, 30));

        let last = trades.last().unwrap();
        assert_eq!(last.bid_trade.order_id, order_id(3));
        assert_eq!(last.ask_trade.order_id, order_id(4));
    }

    #[test]
    fn last_returns_none_when_empty() {
        let trades = Trades::new();
        assert!(trades.last().is_none());
    }

    #[test]
    fn trade_info_fields_accessible() {
        let trade = sample_trade(1, 2, 100, 50);
        assert_eq!(trade.bid_trade.order_id, order_id(1));
        assert_eq!(trade.bid_trade.price, price(100));
        assert_eq!(trade.bid_trade.quantity, qty(50));
        assert_eq!(trade.ask_trade.order_id, order_id(2));
    }
}
