use crate::{Order, OrderError, OrderId, OrderType, Orders, Price, Quantity, Side, Trades};
use std::cmp::Reverse;
use std::collections::BTreeMap;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
struct LevelInfo {
    price: Price,
    quantity: Quantity,
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct LevelInfos(Vec<LevelInfo>);

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct OrderBookLevels {
    bids: LevelInfos,
    asks: LevelInfos,
}

#[derive(Debug, Clone)]
pub struct Orderbook<'a> {
    bids: BTreeMap<Reverse<Price>, Orders>,
    asks: BTreeMap<Price, Orders>,
    orders: Orders,
    trades: Trades<'a>,
}

impl<'a> Orderbook<'a> {
    fn add(&mut self, order: Order) -> Result<(), OrderError> {
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
                } else {
                    let mut orders = Orders::new();
                    orders.push_back(order);
                    self.bids.insert(Reverse(order.price), orders);
                }
            }
            Side::Sell => {
                if let Some(orders) = self.asks.get_mut(&order.price) {
                    orders.push_back(order);
                } else {
                    let mut orders = Orders::new();
                    orders.push_back(order);
                    self.asks.insert(order.price, orders);
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

    fn match_orders(&mut self) {
        // While we have bids and asks
        while let Some(&Reverse(best_bid_price)) = self.bids.keys().next() {
            let best_ask_price = match self.asks.keys().next() {
                Some(&price) => price,
                _ => break,
            };

            // If no overlap we cant match
            if best_bid_price < best_ask_price {
                break;
            }

            if let (Some(bid_orders), Some(ask_orders)) = (
                self.bids.get_mut(&Reverse(best_bid_price)),
                self.asks.get_mut(&best_ask_price),
            ) && let (Some(bid_order), Some(ask_order)) =
                (bid_orders.front_mut(), ask_orders.front_mut())
            {
                let bid_id = bid_order.order_id;
                let ask_id = ask_order.order_id;
                let to_fill =
                    std::cmp::min(bid_order.remaining_quantity, ask_order.remaining_quantity);

                // Fill and remove
                bid_order.fill(to_fill).ok();
                ask_order.fill(to_fill).ok();

                if bid_order.remaining_quantity == Quantity(0) {
                    bid_orders.pop();
                    self.orders.delete(bid_id);
                } else if ask_order.remaining_quantity == Quantity(0) {
                    ask_orders.pop();
                    self.orders.delete(ask_id);
                } else {
                    unimplemented!();
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
}
