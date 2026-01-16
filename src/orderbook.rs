use crate::{
    Order, OrderError, OrderId, OrderType, Orders, Price, Quantity, Side, Trade, TradeInfo, Trades,
};
use std::cmp::Reverse;
use std::collections::BTreeMap;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct LevelInfo {
    price: Price,
    quantity: Quantity,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct LevelInfos(Vec<LevelInfo>);

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct OrderBookLevels {
    bids: LevelInfos,
    asks: LevelInfos,
}

#[derive(Debug, Clone, Default)]
pub struct Orderbook {
    bids: BTreeMap<Reverse<Price>, Orders>,
    asks: BTreeMap<Price, Orders>,
    orders: Orders,
    trades: Trades,
}

impl Orderbook {
    pub fn new() -> Orderbook {
        Orderbook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: Orders::new(),
            trades: Trades::new(),
        }
    }

    pub fn add(&mut self, order: Order) -> Result<(), OrderError> {
        if self.orders.contains(order.order_id) {
            return Err(OrderError::IdExists);
        }

        if order.order_type == OrderType::FillAndKill && !self.can_match(order.side, order.price) {
            return Err(OrderError::CantMatch);
        }

        match order.side {
            Side::Buy => {
                if let Some(orders) = self.bids.get_mut(&Reverse(order.price)) {
                    orders.push_back(order);
                    self.orders.push_back(order);
                } else {
                    let mut orders = Orders::new();
                    orders.push_back(order);
                    self.bids.insert(Reverse(order.price), orders);
                    self.orders.push_back(order);
                }
            }
            Side::Sell => {
                if let Some(orders) = self.asks.get_mut(&order.price) {
                    orders.push_back(order);
                    self.orders.push_back(order);
                } else {
                    let mut orders = Orders::new();
                    orders.push_back(order);
                    self.asks.insert(order.price, orders);
                    self.orders.push_back(order);
                }
            }
        }

        Ok(())
    }
    fn can_match(&self, side: Side, price: Price) -> bool {
        match side {
            Side::Buy => {
                if let Some(&ask_price) = self.asks.keys().next() {
                    ask_price <= price
                } else {
                    false
                }
            }
            Side::Sell => {
                if let Some(&Reverse(bid_price)) = self.bids.keys().next() {
                    bid_price >= price
                } else {
                    false
                }
            }
        }
    }

    pub fn match_orders(&mut self) {
        // While we have bids and asks
        while let (Some(&Reverse(best_bid_price)), Some(&best_ask_price)) =
            (self.bids.keys().next(), self.asks.keys().next())
        {
            if best_bid_price < best_ask_price {
                break;
            }

            // Get order info and fill amount
            let (bid_id, ask_id, to_fill) = {
                let bid_orders = self.bids.get_mut(&Reverse(best_bid_price)).unwrap();
                let ask_orders = self.asks.get_mut(&best_ask_price).unwrap();
                let bid_order = bid_orders.front_mut().unwrap();
                let ask_order = ask_orders.front_mut().unwrap();

                let to_fill =
                    std::cmp::min(bid_order.remaining_quantity, ask_order.remaining_quantity);
                bid_order.fill(to_fill).ok();
                ask_order.fill(to_fill).ok();

                (bid_order.order_id, ask_order.order_id, to_fill)
            }; // borrows end here

            // Record trade
            self.trades.push(Trade {
                bid_trade: TradeInfo::new(bid_id, best_ask_price, to_fill),
                ask_trade: TradeInfo::new(ask_id, best_ask_price, to_fill),
            });

            // Remove filled orders and clean up empty levels
            if let Some(bid_orders) = self.bids.get_mut(&Reverse(best_bid_price)) {
                if bid_orders.front().map(|o| o.is_filled()).unwrap_or(false) {
                    bid_orders.pop();
                    self.orders.delete(bid_id);
                }
                if bid_orders.is_empty() {
                    self.bids.remove(&Reverse(best_bid_price));
                }
            }

            if let Some(ask_orders) = self.asks.get_mut(&best_ask_price) {
                if ask_orders.front().map(|o| o.is_filled()).unwrap_or(false) {
                    ask_orders.pop();
                    self.orders.delete(ask_id);
                }
                if ask_orders.is_empty() {
                    self.asks.remove(&best_ask_price);
                }
            }
        }
        // We need to remove FillAndKills with no other side
        let bid_fak_ids: Vec<OrderId> = self
            .bids
            .first_key_value()
            .map(|(_, orders)| {
                orders
                    .iter()
                    .filter(|order| order.order_type == OrderType::FillAndKill)
                    .map(|order| order.order_id)
                    .collect()
            })
            .unwrap_or_default();

        let ask_fak_ids: Vec<OrderId> = self
            .asks
            .first_key_value()
            .map(|(_, orders)| {
                orders
                    .iter()
                    .filter(|order| order.order_type == OrderType::FillAndKill)
                    .map(|order| order.order_id)
                    .collect()
            })
            .unwrap_or_default();

        for id in bid_fak_ids.into_iter().chain(ask_fak_ids) {
            let _ = self.cancel_order(id);
        }
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<(), OrderError> {
        let (side, price) = {
            let order = self.orders.get(order_id).ok_or(OrderError::OrderNotFound)?;
            (order.side, order.price)
        };

        self.orders.delete(order_id);

        match side {
            Side::Buy => {
                if let Some(bids_at_price) = self.bids.get_mut(&Reverse(price)) {
                    bids_at_price.delete(order_id);
                    if bids_at_price.is_empty() {
                        self.bids.remove(&Reverse(price));
                    }
                }
            }
            Side::Sell => {
                if let Some(asks_at_price) = self.asks.get_mut(&price) {
                    asks_at_price.delete(order_id);
                    if asks_at_price.is_empty() {
                        self.asks.remove(&price);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_levels(&self) -> OrderBookLevels {
        let bids = LevelInfos(
            self.bids
                .iter()
                .map(|(Reverse(price), orders)| LevelInfo {
                    price: *price,
                    quantity: orders
                        .iter()
                        .map(|order| order.remaining_quantity)
                        .fold(Quantity(0), |acc, q| Quantity(acc.0 + q.0)),
                })
                .collect(),
        );

        let asks = LevelInfos(
            self.asks
                .iter()
                .map(|(price, orders)| LevelInfo {
                    price: *price,
                    quantity: orders
                        .iter()
                        .map(|order| order.remaining_quantity)
                        .fold(Quantity(0), |acc, q| Quantity(acc.0 + q.0)),
                })
                .collect(),
        );

        OrderBookLevels { bids, asks }
    }

    pub fn trades(&self) -> &Trades {
        &self.trades
    }

    pub fn clear_trades(&mut self) {
        self.trades.clear();
    }
}

impl OrderBookLevels {
    pub fn bids(&self) -> &[LevelInfo] {
        &self.bids.0
    }
    pub fn asks(&self) -> &[LevelInfo] {
        &self.asks.0
    }
}

impl LevelInfo {
    pub fn price(&self) -> Price {
        self.price
    }
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OrderId, OrderType, Quantity, Side};

    fn price(p: u32) -> Price {
        Price::new(p)
    }

    fn qty(q: u32) -> Quantity {
        Quantity(q)
    }

    fn order_id(id: u64) -> OrderId {
        OrderId::new(id)
    }

    fn buy_order(id: u64, p: u32, q: u32) -> Order {
        Order::new(
            order_id(id),
            OrderType::GoodTillCancelled,
            Side::Buy,
            price(p),
            qty(q),
        )
    }

    fn sell_order(id: u64, p: u32, q: u32) -> Order {
        Order::new(
            order_id(id),
            OrderType::GoodTillCancelled,
            Side::Sell,
            price(p),
            qty(q),
        )
    }

    fn buy_fak(id: u64, p: u32, q: u32) -> Order {
        Order::new(
            order_id(id),
            OrderType::FillAndKill,
            Side::Buy,
            price(p),
            qty(q),
        )
    }

    #[test]
    fn new_orderbook_is_empty() {
        let ob = Orderbook::new();
        let levels = ob.get_levels();
        assert!(levels.bids.0.is_empty());
        assert!(levels.asks.0.is_empty());
    }

    #[test]
    fn add_buy_order() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        let levels = ob.get_levels();
        assert_eq!(levels.bids.0.len(), 1);
        assert_eq!(levels.bids.0[0].price, price(100));
        assert_eq!(levels.bids.0[0].quantity, qty(50));
    }

    #[test]
    fn add_sell_order() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap();
        let levels = ob.get_levels();
        assert_eq!(levels.asks.0.len(), 1);
        assert_eq!(levels.asks.0[0].price, price(100));
        assert_eq!(levels.asks.0[0].quantity, qty(50));
    }

    #[test]
    fn add_duplicate_id_fails() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        let result = ob.add(buy_order(1, 110, 60));
        assert!(matches!(result, Err(OrderError::IdExists)));
    }

    #[test]
    fn add_multiple_orders_same_price() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(buy_order(2, 100, 30)).unwrap();
        let levels = ob.get_levels();
        assert_eq!(levels.bids.0.len(), 1);
        assert_eq!(levels.bids.0[0].quantity, qty(80));
    }

    #[test]
    fn add_multiple_orders_different_prices() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(buy_order(2, 110, 30)).unwrap();
        ob.add(buy_order(3, 90, 20)).unwrap();
        let levels = ob.get_levels();
        assert_eq!(levels.bids.0.len(), 3);
        assert_eq!(levels.bids.0[0].price, price(110));
        assert_eq!(levels.bids.0[1].price, price(100));
        assert_eq!(levels.bids.0[2].price, price(90));
    }

    #[test]
    fn asks_sorted_low_to_high() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 110, 30)).unwrap();
        ob.add(sell_order(3, 90, 20)).unwrap();
        let levels = ob.get_levels();
        assert_eq!(levels.asks.0.len(), 3);
        assert_eq!(levels.asks.0[0].price, price(90));
        assert_eq!(levels.asks.0[1].price, price(100));
        assert_eq!(levels.asks.0[2].price, price(110));
    }

    #[test]
    fn can_match_buy_when_ask_exists_at_or_below() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap();
        assert!(ob.can_match(Side::Buy, price(100)));
        assert!(ob.can_match(Side::Buy, price(110)));
        assert!(!ob.can_match(Side::Buy, price(90)));
    }

    #[test]
    fn can_match_sell_when_bid_exists_at_or_above() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        assert!(ob.can_match(Side::Sell, price(100)));
        assert!(ob.can_match(Side::Sell, price(90)));
        assert!(!ob.can_match(Side::Sell, price(110)));
    }

    #[test]
    fn can_match_false_on_empty_book() {
        let ob = Orderbook::new();
        assert!(!ob.can_match(Side::Buy, price(100)));
        assert!(!ob.can_match(Side::Sell, price(100)));
    }

    #[test]
    fn fak_rejected_when_no_match_possible() {
        let mut ob = Orderbook::new();
        let result = ob.add(buy_fak(1, 100, 50));
        assert!(matches!(result, Err(OrderError::CantMatch)));
    }

    #[test]
    fn fak_rejected_when_price_doesnt_cross() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 110, 50)).unwrap();
        let result = ob.add(buy_fak(2, 100, 50));
        assert!(matches!(result, Err(OrderError::CantMatch)));
    }

    #[test]
    fn fak_accepted_when_can_match() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap();
        let result = ob.add(buy_fak(2, 100, 50));
        assert!(result.is_ok());
    }
}
